use parking_lot::RwLock;
use std::{
    collections::HashMap,
    fmt::Debug,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

use expression::{TypecheckedExpression, TypedLiteral};
use types::resolve_primitive_type;

use crate::{
    annotations::Annotations,
    globals::GlobalStr,
    lang_items::LangItems,
    module::{FunctionId, ModuleContext, ModuleId, ModuleScopeValue, StructId, TraitId},
    parser::TypeRef,
    tokenizer::Location,
};

mod error;
pub mod expression;
pub mod intrinsics;
pub mod ir_displayer;
mod type_resolution;
pub mod typechecking;
mod types;
pub use error::TypecheckingError;
pub use types::Type;

pub static DUMMY_LOCATION: LazyLock<Location> = LazyLock::new(|| Location {
    line: 0,
    column: 0,
    file: PathBuf::from("").into(), // a file should never be a folder :3
});

#[derive(Debug)]
pub struct TypecheckedFunctionContract {
    pub name: Option<GlobalStr>,
    pub arguments: Vec<(GlobalStr, Type)>,
    pub return_type: Type,
    pub annotations: Annotations,
    pub location: Location,
    pub module_id: ModuleId,
}

impl Hash for TypecheckedFunctionContract {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.name {
            None => "{{anon_fn}}".hash(state),
            Some(v) => v.hash(state),
        }
        self.module_id.hash(state);
        self.arguments.hash(state);
        self.return_type.hash(state);
    }
}

#[derive(Debug)]
pub struct TypedTrait {
    pub name: GlobalStr,
    pub functions: Vec<(
        GlobalStr,
        Vec<(GlobalStr, Type)>,
        Type,
        Annotations,
        Location,
    )>,
    pub location: Location,
    pub module_id: ModuleId,
    pub id: TraitId,
    pub annotations: Annotations,
}

#[derive(Debug)]
pub struct TypedStruct {
    pub name: GlobalStr,
    pub elements: Vec<(GlobalStr, Type)>,
    pub location: Location,
    pub global_impl: HashMap<GlobalStr, FunctionId>,
    pub trait_impl: HashMap<TraitId, Vec<FunctionId>>,
    pub annotations: Annotations,
    pub module_id: ModuleId,
    pub id: StructId,
    pub generics: Vec<(GlobalStr, Vec<TraitId>)>,
}

impl Hash for TypedStruct {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.elements.hash(state);
        self.module_id.hash(state);
    }
}

#[derive(Debug)]
pub struct TypecheckingContext {
    pub modules: RwLock<Vec<TypecheckedModule>>,
    pub functions: RwLock<Vec<(TypecheckedFunctionContract, Box<[TypecheckedExpression]>)>>,
    pub external_functions: RwLock<
        Vec<(
            TypecheckedFunctionContract,
            Option<Box<[TypecheckedExpression]>>,
        )>,
    >,
    pub statics: RwLock<
        Vec<(
            Type,
            TypedLiteral, /* guaranteed to not be `Dynamic`, `Intrinsic` or `Static` */
            ModuleId,
            Location,
            Annotations,
        )>,
    >,
    pub structs: RwLock<Vec<TypedStruct>>,
    pub traits: RwLock<Vec<TypedTrait>>,
    pub lang_items: RwLock<LangItems>,
}

pub struct TypecheckedModule {
    context: Arc<TypecheckingContext>,
    scope: HashMap<GlobalStr, ModuleScopeValue>,
    exports: HashMap<GlobalStr, GlobalStr>,
    pub path: Arc<Path>,
    pub root: Arc<Path>,
}

impl Debug for TypecheckedModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypecheckedModule")
            .field("scope", &self.scope)
            .finish()
    }
}

impl TypecheckingContext {
    pub fn new(context: Arc<ModuleContext>) -> Arc<Self> {
        let modules = RwLock::new(Vec::new());
        let traits_reader = context.traits.read();
        let structs_reader = context.structs.read();
        let statics_reader = context.statics.read();
        let functions_reader = context.functions.read();
        let external_functions_reader = context.external_functions.read();
        let num_traits = traits_reader.len();
        let num_structs = structs_reader.len();
        let num_statics = statics_reader.len();
        let num_functions = functions_reader.len();
        let num_external_functions = external_functions_reader.len();

        let mut traits = Vec::with_capacity(num_traits);
        let mut structs = Vec::with_capacity(num_structs);
        let mut statics = Vec::with_capacity(num_statics);
        let mut functions = Vec::with_capacity(num_functions);
        let mut external_functions = Vec::with_capacity(num_external_functions);

        for id in 0..num_structs {
            structs.push(TypedStruct {
                name: GlobalStr::ZERO,
                elements: Vec::new(),
                location: DUMMY_LOCATION.clone(),
                global_impl: HashMap::new(),
                trait_impl: HashMap::new(),
                annotations: Annotations::default(),
                module_id: 0,
                generics: Vec::new(),
                id,
            });
        }

        for _ in 0..num_statics {
            statics.push((
                Type::PrimitiveNever,
                TypedLiteral::Void,
                0,
                DUMMY_LOCATION.clone(),
                Annotations::default(),
            ));
        }

        for _ in 0..num_functions {
            functions.push((
                TypecheckedFunctionContract {
                    annotations: Annotations::default(),
                    name: None,
                    arguments: Vec::new(),
                    return_type: Type::PrimitiveNever,
                    location: DUMMY_LOCATION.clone(),
                    module_id: 0,
                },
                vec![].into_boxed_slice(),
            ));
        }

        for _ in 0..num_external_functions {
            external_functions.push((
                TypecheckedFunctionContract {
                    annotations: Annotations::default(),
                    name: None,
                    arguments: Vec::new(),
                    return_type: Type::PrimitiveNever,
                    location: DUMMY_LOCATION.clone(),
                    module_id: 0,
                },
                None,
            ))
        }

        for _ in 0..num_traits {
            traits.push(TypedTrait {
                name: GlobalStr::ZERO,
                functions: Vec::new(),
                location: DUMMY_LOCATION.clone(),
                module_id: 0,
                id: 0,
                annotations: Annotations::default(),
            });
        }

        let me = Arc::new(Self {
            structs: structs.into(),
            statics: statics.into(),
            functions: functions.into(),
            traits: traits.into(),
            external_functions: external_functions.into(),
            modules,
            lang_items: RwLock::new(LangItems::default()),
        });

        let mut typechecked_module_writer = me.modules.write();
        let module_reader = context.modules.read();

        let module_id = typechecked_module_writer.len();
        let scope = module_reader[module_id].scope.clone();

        typechecked_module_writer.push(TypecheckedModule {
            context: me.clone(),
            scope,
            exports: module_reader[module_id].exports.clone(),
            path: module_reader[module_id].path.clone(),
            root: module_reader[module_id].root.clone(),
        });

        drop(module_reader);
        drop(typechecked_module_writer);

        me
    }

    pub fn resolve_imports(&self, context: Arc<ModuleContext>) -> Vec<TypecheckingError> {
        let mut errors = vec![];
        let mut typechecked_module_writer = self.modules.write();
        let module_reader = context.modules.read();
        for id in 0..typechecked_module_writer.len() {
            for (name, (location, module_id, path)) in module_reader[id].imports.iter() {
                match resolve_import(&context, *module_id, path, location, &mut Vec::new()) {
                    Err(e) => errors.push(e),
                    Ok(k) => {
                        typechecked_module_writer[id].scope.insert(name.clone(), k);
                    }
                }
            }
        }

        errors
    }

    pub fn resolve_type(
        &self,
        module_id: ModuleId,
        typ: &TypeRef,
        generics: &[GlobalStr],
    ) -> Result<Type, TypecheckingError> {
        if let Some(primitive) = resolve_primitive_type(typ) {
            return Ok(primitive);
        }

        match typ {
            TypeRef::DynReference { .. } => todo!(),
            TypeRef::Reference {
                num_references,
                type_name,
                loc,
            } => {
                if type_name.entries.len() == 1 && type_name.entries[0].1.len() == 0 {
                    if generics.contains(&type_name.entries[0].0) {
                        return Ok(Type::Generic(
                            type_name.entries[0].0.clone(),
                            *num_references,
                        ));
                    }
                }

                let path = type_name
                    .entries
                    .iter()
                    .map(|v| v.0.clone())
                    .collect::<Vec<_>>();
                // NOTE: this should only have a generic at the end as this is a type
                // (std::vec::Vec, can never be std::vec::Vec<u32>::Vec.)
                for (_, generics) in type_name.entries.iter() {
                    if generics.len() > 0 {
                        return Err(TypecheckingError::UnexpectedGenerics {
                            location: loc.clone(),
                        });
                    }
                }

                match typed_resolve_import(self, module_id, &path, loc, &mut Vec::new())? {
                    ModuleScopeValue::Struct(id) => Ok(Type::Struct {
                        struct_id: id,
                        name: self.structs.read()[id].name.clone(),
                        num_references: *num_references,
                    }),
                    v => Err(TypecheckingError::MismatchingScopeType {
                        location: loc.clone(),
                        expected: ScopeKind::Type,
                        found: v.into(),
                    }),
                }
            }
            TypeRef::Void(..) | TypeRef::Never(_) => unreachable!(),
            TypeRef::UnsizedArray {
                num_references,
                child,
                loc: _,
            } => Ok(Type::UnsizedArray {
                typ: Box::new(self.resolve_type(module_id, &**child, generics)?),
                num_references: *num_references,
            }),
            TypeRef::SizedArray {
                num_references,
                child,
                number_elements,
                loc: _,
            } => Ok(Type::SizedArray {
                typ: Box::new(self.resolve_type(module_id, &**child, generics)?),
                num_references: *num_references,
                number_elements: *number_elements,
            }),
        }
    }

    /// returns if a recursive field was detected
    fn resolve_struct(
        &self,
        context: Arc<ModuleContext>,
        id: StructId,
        module_id: ModuleId,
        errors: &mut Vec<TypecheckingError>,
    ) -> bool {
        if DUMMY_LOCATION.ne(&self.structs.read()[id].location) {
            return false;
        }

        let mut writer = context.structs.write();
        if writer[id].location == *DUMMY_LOCATION {
            return true;
        }

        let global_impl = std::mem::take(&mut writer[id].global_impl);
        let annotations = std::mem::take(&mut writer[id].annotations);
        let elements = std::mem::take(&mut writer[id].elements);
        let mut generics = Vec::new();

        for generic in &writer[id].generics {
            let mut bounds = Vec::new();

            for (bound, loc) in &generic.bounds {
                match resolve_import(&context, module_id, &bound.entries, loc, &mut Vec::new()) {
                    Err(e) => errors.push(e),
                    Ok(ModuleScopeValue::Trait(trait_id)) => bounds.push(trait_id),
                    Ok(_) => errors.push(TypecheckingError::UnboundIdent {
                        location: loc.clone(),
                        name: bound.entries[bound.entries.len() - 1].clone(),
                    }),
                }
            }

            generics.push((generic.name.clone(), bounds));
        }

        let mut typed_struct = TypedStruct {
            name: writer[id].name.clone(),
            location: std::mem::replace(&mut writer[id].location, DUMMY_LOCATION.clone()),
            elements: Vec::new(),
            global_impl,
            annotations,
            module_id,
            id,
            generics,
            trait_impl: HashMap::new(),
        };
        drop(writer);

        for element in elements {
            if let Some(typ) = self.type_resolution_resolve_type(
                &element.1,
                |generic_name| {
                    typed_struct
                        .generics
                        .iter()
                        .find(|(v, ..)| *v == *generic_name)
                        .is_some()
                },
                module_id,
                context.clone(),
                errors,
            ) {
                let typ = match typ {
                    Type::Generic(real_name, num_references) => {
                        match typed_struct.generics.iter().find(|(v, ..)| *v == real_name) {
                            Some(v) if v.1.len() > 0 => Type::Trait {
                                trait_refs: v.1.clone(),
                                num_references,
                                real_name,
                            },
                            _ => Type::Generic(real_name, num_references),
                        }
                    }
                    t => t,
                };
                typed_struct.elements.push((element.0, typ));
            }
        }
        self.structs.write()[id] = typed_struct;

        false
    }

    fn type_resolution_resolve_type<F: Fn(&GlobalStr) -> bool>(
        &self,
        typ: &TypeRef,
        is_generic_name: F,
        module: ModuleId,
        context: Arc<ModuleContext>,
        errors: &mut Vec<TypecheckingError>,
    ) -> Option<Type> {
        if let Some(typ) = resolve_primitive_type(typ) {
            return Some(typ);
        }
        match typ {
            TypeRef::DynReference { .. } => todo!(),
            TypeRef::Reference {
                num_references,
                type_name,
                loc,
            } => {
                let path = type_name
                    .entries
                    .iter()
                    .map(|v| v.0.clone())
                    .collect::<Vec<_>>();
                // NOTE: this should only have a generic at the end as this is a type
                // (std::vec::Vec, can never be std::vec::Vec<u32>::Vec.)
                for (_, generics) in type_name.entries.iter() {
                    if generics.len() > 0 {
                        return None;
                    }
                }

                // generics can never have a generic attribute (struct Moew<T> { value: T<u32> })
                if type_name.entries.len() == 1 && type_name.entries[0].1.len() == 0 {
                    if is_generic_name(&type_name.entries[0].0) {
                        return Some(Type::Generic(type_name.entries[0].0.clone(), 0));
                    }
                }

                let Ok(value) = resolve_import(&context, module, &path, loc, &mut Vec::new())
                else {
                    errors.push(TypecheckingError::UnboundIdent {
                        location: loc.clone(),
                        name: path[path.len() - 1].clone(),
                    });
                    return None;
                };

                let ModuleScopeValue::Struct(id) = value else {
                    errors.push(TypecheckingError::MismatchingScopeType {
                        location: loc.clone(),
                        expected: ScopeKind::Type,
                        found: value.into(),
                    });
                    return None;
                };

                {
                    let typechecked_struct = &self.structs.read()[id];
                    if typechecked_struct.location != *DUMMY_LOCATION {
                        return Some(Type::Struct {
                            struct_id: typechecked_struct.id,
                            name: typechecked_struct.name.clone(),
                            num_references: *num_references,
                        });
                    }
                }

                let module = context.structs.read()[id].module_id;
                if self.resolve_struct(context, id, module, errors) {
                    errors.push(TypecheckingError::RecursiveTypeDetected {
                        location: loc.clone(),
                    });
                    return None;
                }
                let typechecked_struct = &self.structs.read()[id];
                if typechecked_struct.location != *DUMMY_LOCATION {
                    return Some(Type::Struct {
                        struct_id: typechecked_struct.id,
                        num_references: *num_references,
                        name: typechecked_struct.name.clone(),
                    });
                }
                unreachable!("struct should be resolved by here")
            }
            TypeRef::Void(..) => unreachable!(),
            TypeRef::Never(_) => unreachable!(),
            TypeRef::UnsizedArray {
                num_references,
                child,
                loc: _,
            } => Some(Type::UnsizedArray {
                typ: Box::new(self.type_resolution_resolve_type(
                    child,
                    is_generic_name,
                    module,
                    context,
                    errors,
                )?),
                num_references: *num_references,
            }),
            TypeRef::SizedArray {
                num_references,
                child,
                number_elements,
                loc: _,
            } => Some(Type::SizedArray {
                typ: Box::new(self.type_resolution_resolve_type(
                    child,
                    is_generic_name,
                    module,
                    context,
                    errors,
                )?),
                num_references: *num_references,
                number_elements: *number_elements,
            }),
        }
    }
}

fn typed_resolve_import(
    context: &TypecheckingContext,
    module: ModuleId,
    import: &[GlobalStr],
    location: &Location,
    already_included: &mut Vec<(ModuleId, GlobalStr)>,
) -> Result<ModuleScopeValue, TypecheckingError> {
    if import.len() < 1 {
        return Ok(ModuleScopeValue::Module(module));
    }
    if already_included
        .iter()
        .find(|(mod_id, imp)| module.eq(mod_id) && import[0].eq(imp))
        .is_some()
    {
        return Err(TypecheckingError::CyclicDependency {
            location: location.clone(),
        });
    }
    already_included.push((module, import[0].clone()));

    let reader = context.modules.read();
    let ident = match reader[module].exports.get(&import[0]) {
        Some(ident) => ident,
        None if already_included.len() < 2 /* this is the module it was imported from */ => &import[0],
        None => return Err(TypecheckingError::ExportNotFound {
            location: location.clone(),
            name: import[0].clone(),
        }),
    };

    if let Some(value) = reader[module].scope.get(ident).copied() {
        if import.len() < 2 {
            return Ok(value);
        }
        match value {
            ModuleScopeValue::Struct(id) => {
                let reader = context.structs.read();
                if let Some(function_id) = reader[id].global_impl.get(&import[1]).copied() {
                    if import.len() < 3 {
                        return Ok(ModuleScopeValue::Function(function_id));
                    }
                    return Err(TypecheckingError::ExportNotFound {
                        location: location.clone(),
                        name: import[2].clone(),
                    });
                } else {
                    return Err(TypecheckingError::ExportNotFound {
                        location: reader[id].location.clone(),
                        name: import[1].clone(),
                    });
                }
            }
            ModuleScopeValue::Module(_) => unreachable!(), // all modules must have been imports
            ModuleScopeValue::Function(_)
            | ModuleScopeValue::ExternalFunction(_)
            | ModuleScopeValue::Trait(_)
            | ModuleScopeValue::Static(_) => {
                return Err(TypecheckingError::ExportNotFound {
                    location: location.clone(),
                    name: import[1].clone(),
                })
            }
        }
    }
    Err(TypecheckingError::ExportNotFound {
        location: location.clone(),
        name: import[0].clone(),
    })
}

fn resolve_import(
    context: &ModuleContext,
    module: ModuleId,
    import: &[GlobalStr],
    location: &Location,
    already_included: &mut Vec<(ModuleId, GlobalStr)>,
) -> Result<ModuleScopeValue, TypecheckingError> {
    if import.len() < 1 {
        return Ok(ModuleScopeValue::Module(module));
    }
    if already_included
        .iter()
        .find(|(mod_id, imp)| module.eq(mod_id) && import[0].eq(imp))
        .is_some()
    {
        return Err(TypecheckingError::CyclicDependency {
            location: location.clone(),
        });
    }
    already_included.push((module, import[0].clone()));

    let reader = context.modules.read();
    let ident = match reader[module].exports.get(&import[0]) {
        Some(ident) => ident,
        None if already_included.len() < 2 /* this is the module it was imported from */ => &import[0],
        None => return Err(TypecheckingError::ExportNotFound {
            location: location.clone(),
            name: import[0].clone(),
        }),
    };

    if let Some((sub_location, module, path)) = reader[module].imports.get(ident) {
        let value = resolve_import(context, *module, path, sub_location, already_included)?;
        if import.len() < 2 {
            return Ok(value);
        }

        match value {
            ModuleScopeValue::Module(id) => {
                return resolve_import(context, id, &import[1..], location, already_included)
            }
            ModuleScopeValue::Struct(id) => {
                let reader = context.structs.read();
                if let Some(function_id) = reader[id].global_impl.get(&import[1]).copied() {
                    if import.len() < 3 {
                        return Ok(ModuleScopeValue::Function(function_id));
                    }
                    return Err(TypecheckingError::ExportNotFound {
                        location: context.functions.read()[function_id].0.location.clone(),
                        name: import[2].clone(),
                    });
                } else {
                    return Err(TypecheckingError::ExportNotFound {
                        location: reader[id].location.clone(),
                        name: import[1].clone(),
                    });
                }
            }
            ModuleScopeValue::Function(_)
            | ModuleScopeValue::ExternalFunction(_)
            | ModuleScopeValue::Trait(_)
            | ModuleScopeValue::Static(_) => {
                return Err(TypecheckingError::ExportNotFound {
                    location: location.clone(),
                    name: import[1].clone(),
                })
            }
        }
    }
    if let Some(value) = reader[module].scope.get(ident).copied() {
        if import.len() < 2 {
            return Ok(value);
        }
        match value {
            ModuleScopeValue::Struct(id) => {
                let reader = context.structs.read();
                if let Some(function_id) = reader[id].global_impl.get(&import[1]).copied() {
                    if import.len() < 3 {
                        return Ok(ModuleScopeValue::Function(function_id));
                    }
                    return Err(TypecheckingError::ExportNotFound {
                        location: location.clone(),
                        name: import[2].clone(),
                    });
                } else {
                    return Err(TypecheckingError::ExportNotFound {
                        location: reader[id].location.clone(),
                        name: import[1].clone(),
                    });
                }
            }
            ModuleScopeValue::Module(_) => unreachable!(), // all modules must have been imports
            ModuleScopeValue::Function(_)
            | ModuleScopeValue::ExternalFunction(_)
            | ModuleScopeValue::Trait(_)
            | ModuleScopeValue::Static(_) => {
                return Err(TypecheckingError::ExportNotFound {
                    location: location.clone(),
                    name: import[1].clone(),
                })
            }
        }
    }
    Err(TypecheckingError::ExportNotFound {
        location: location.clone(),
        name: import[1].clone(),
    })
}

impl From<ModuleScopeValue> for ScopeKind {
    fn from(value: ModuleScopeValue) -> Self {
        match value {
            ModuleScopeValue::Trait(_) => Self::Trait,
            ModuleScopeValue::Struct(_) => Self::Type,
            ModuleScopeValue::Static(_) => Self::Static,
            ModuleScopeValue::Module(_) => Self::Module,
            ModuleScopeValue::Function(_) | ModuleScopeValue::ExternalFunction(_) => Self::Function,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ScopeKind {
    Trait,
    Type,
    Function,
    Static,
    Module,
}

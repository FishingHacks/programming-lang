use std::collections::HashMap;

use crate::{
    error::ProgrammingLangProgramFormingError,
    globals::GlobalStr,
    parser::{
        Expression, FunctionContract, Implementation, LiteralValue, Statement, Struct, TypeRef,
    },
    tokenizer::Location,
};

pub type FunctionId = usize;

#[derive(Debug)]
pub struct Module {
    pub structs: HashMap<GlobalStr, Struct>,
    pub functions: HashMap<GlobalStr, FunctionId>,
    pub external_functions: HashMap<GlobalStr, FunctionContract>,
    pub static_values: HashMap<GlobalStr, (TypeRef, LiteralValue)>,
    pub function_registry: Vec<(FunctionContract, Statement)>,
    pub imports: HashMap<GlobalStr, (Location, usize, Vec<GlobalStr>)>,
    pub exports: HashMap<GlobalStr, GlobalStr>,
}

impl Module {
    pub fn new(imports: HashMap<GlobalStr, (Location, usize, Vec<GlobalStr>)>) -> Self {
        Self {
            structs: HashMap::new(),
            functions: HashMap::new(),
            external_functions: HashMap::new(),
            static_values: HashMap::new(),

            function_registry: Vec::new(),
            imports,
            exports: HashMap::new(),
        }
    }

    pub fn push_fn(&mut self, contract: FunctionContract, statement: Statement) -> FunctionId {
        self.function_registry.push((contract, statement));
        return (self.function_registry.len() - 1) as FunctionId;
    }

    pub fn get_fn(&self, id: FunctionId) -> Option<&(FunctionContract, Statement)> {
        return self.function_registry.get(id);
    }

    pub fn push_all(
        &mut self,
        statements: Vec<Statement>,
    ) -> Result<(), Vec<ProgrammingLangProgramFormingError>> {
        let errors = statements
            .into_iter()
            .map(|statement| self.push_statement(statement))
            .filter_map(|el| el.err())
            .collect::<Vec<_>>();

        if errors.len() > 0 {
            Err(errors)
        } else {
            Ok(())
        }
    }

    pub fn push_statement(
        &mut self,
        statement: Statement,
    ) -> Result<(), ProgrammingLangProgramFormingError> {
        let loc = statement.loc().clone();
        match statement {
            Statement::Function(contract, mut statement) => {
                let Some(name) = contract.name.clone() else {
                    return Err(
                        ProgrammingLangProgramFormingError::AnonymousFunctionAtGlobalLevel(loc),
                    );
                };

                statement.bake_functions(self);
                let fn_id = self.push_fn(contract, *statement);
                self.functions.insert(name, fn_id);
            }
            Statement::Struct {
                name,
                elements: fields,
                location,
                global_impl,
                impls,
                annotations,
            } => {
                if self.is_defined(&name) {
                    return Err(ProgrammingLangProgramFormingError::IdentAlreadyDefined(
                        location, name,
                    ));
                }

                let mut struct_global_impl: Implementation = HashMap::new();

                for (function_name, mut function) in global_impl.into_iter() {
                    function.bake(self);
                    struct_global_impl.insert(function_name, function.get_baked_id());
                }

                let mut struct_impls: Vec<(GlobalStr, Implementation)> = Vec::new();

                for (trait_name, trait_impl) in impls.into_iter() {
                    let mut cur_impl: Implementation = HashMap::new();

                    for (function_name, mut function) in trait_impl.into_iter() {
                        function.bake(self);
                        cur_impl.insert(function_name, function.get_baked_id());
                    }

                    struct_impls.push((trait_name, cur_impl));
                }

                let typ = Struct {
                    loc: location,
                    name: name.clone(),
                    fields,
                    global_impl: struct_global_impl,
                    trait_impls: struct_impls,
                    annotations,
                };
                self.structs.insert(name, typ);
            }
            Statement::Var(_, expr, None, _) => {
                return Err(ProgrammingLangProgramFormingError::GlobalValueNoType(
                    expr.loc().clone(),
                ))
            }
            Statement::Var(name, expr, Some(typ), location) => {
                if self.is_defined(&name) {
                    return Err(ProgrammingLangProgramFormingError::IdentAlreadyDefined(
                        location, name,
                    ));
                }
                if let Expression::Literal(val, _) = expr {
                    self.static_values.insert(name, (typ, val));
                } else {
                    return Err(ProgrammingLangProgramFormingError::GlobalValueNoLiteral(
                        expr.loc().clone(),
                    ));
                }
            }
            Statement::ExternalFunction(contract) => {
                if let Some(name) = contract.name.clone() {
                    if self.is_defined(&name) {
                        return Err(ProgrammingLangProgramFormingError::IdentAlreadyDefined(
                            contract.location.clone(),
                            name,
                        ));
                    }
                    self.external_functions.insert(name.clone(), contract);
                } else {
                    return Err(
                        ProgrammingLangProgramFormingError::AnonymousFunctionAtGlobalLevel(
                            contract.location,
                        ),
                    );
                }
            }
            Statement::Export(key, exported_key, loc) => {
                if !self.is_defined(&key) {
                    return Err(ProgrammingLangProgramFormingError::IdentNotDefined(
                        loc, key,
                    ));
                }
                self.exports.insert(exported_key, key);
            }
            _ => return Err(ProgrammingLangProgramFormingError::NoCodeOutsideOfFunctions(loc)),
        }

        Ok(())
    }

    fn is_defined(&self, key: &GlobalStr) -> bool {
        self.imports.contains_key(key)
            || self.functions.contains_key(key)
            || self.structs.contains_key(key)
            || self.static_values.contains_key(key)
            || self.external_functions.contains_key(key)
    }
}

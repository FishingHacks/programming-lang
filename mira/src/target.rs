use std::{
    fmt::{Display, Write},
    str::FromStr,
};

use inkwell::targets::TargetTriple;
use thiserror::Error;

macro_rules! str_enum {
    ($name:ident: $($tag:ident = $value:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name {
            $($tag),*
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.to_str()) }
        }

        impl std::str::FromStr for $name {
            type Err = ();

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($value => Ok(Self::$tag),)*
                    _ => Err(()),
                }
            }
        }

        impl $name {
            fn to_str(&self) -> &str {
                match self {
                    $(Self::$tag => $value,)*
                }
            }
        }
    };
}

str_enum! {
Os:
    Freestanding = "freestanding",
    Other = "other",
    Linux = "linux",
}

impl Os {
    pub fn exe_file_ext(&self) -> &str {
        match self {
            _ => "",
        }
    }

    pub fn dynamic_lib_ext(&self) -> &str {
        match self {
            _ => "so",
        }
    }

    pub fn to_llvm(&self) -> &str {
        match self {
            Os::Freestanding | Os::Other => "unknown",
            Os::Linux => "pc-linux",
        }
    }
}

str_enum! {
Arch:
    X86_64 = "x86_64",
    X86 = "x86",
}

impl Arch {
    pub fn endianess(&self) -> Endianess {
        match self {
            Self::X86 | Self::X86_64 => Endianess::Big,
        }
    }

    pub fn generic_name(&self) -> &str {
        match self {
            Self::X86 | Self::X86_64 => "x86",
        }
    }

    pub fn is_x86(&self) -> bool {
        matches!(self, Self::X86 | Self::X86_64)
    }

    pub fn to_llvm_cpu(&self) -> &str {
        match self {
            Arch::X86_64 => "x86-64",
            Arch::X86 => "x86",
        }
    }

    pub fn to_llvm(&self) -> &str {
        match self {
            Arch::X86_64 | Arch::X86 => self.to_str(),
        }
    }
}

str_enum! {
Abi:
    None = "none",
    Gnu = "gnu",
}

impl Abi {
    pub fn to_llvm(&self) -> &str {
        match self {
            Abi::None | Abi::Gnu => self.to_str(),
        }
    }
}

str_enum! {
Endianess:
    Big = "big",
    Little = "little",
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Target {
    pub arch: Arch,
    pub os: Os,
    pub abi: Abi,
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.arch, f)?;
        f.write_char('-')?;
        Display::fmt(&self.os, f)?;
        if self.abi != Abi::None {
            f.write_char('-')?;
            Display::fmt(&self.abi, f)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Target(")?;
        Display::fmt(&self, f)?;
        f.write_char(')')
    }
}

#[derive(Error, Clone, Copy, Debug)]
pub enum TargetParsingError {
    #[error("Invalid Arch")]
    InvalidArch,
    #[error("Invalid Operating System")]
    InvalidOs,
    #[error("Invalid ABI")]
    InvalidAbi,
    #[error("Too many arguments. Format: arch-os-abi or arch-os")]
    TooManyArguments,
    #[error("No arch specified. Format: arch-os-abi or arch-os")]
    MissingArch,
    #[error("No os specified. Format: arch-os-abi or arch-os")]
    MissingOs,
}

impl Target {
    pub fn to_llvm(&self) -> String {
        let mut s = self.arch.to_llvm().to_string();
        s.push('-');
        s.push_str(self.os.to_llvm());
        s.push('-');
        s.push_str(self.abi.to_llvm());
        s
    }

    pub fn new(arch: Arch, os: Os, abi: Abi) -> Self {
        Self { arch, os, abi }
    }

    pub fn new_simple(arch: Arch, os: Os) -> Self {
        Self::new(arch, os, Abi::None)
    }

    pub fn to_llvm_triple(&self) -> (TargetTriple, String) {
        let v = self.to_llvm();
        (TargetTriple::create(&v), v)
    }

    pub fn from_name(name: &str) -> Self {
        Target::from_str(name).expect("failed to parse target")
    }
}

impl std::str::FromStr for Target {
    type Err = TargetParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.split('-');
        let arch = iter.next().ok_or(TargetParsingError::MissingArch)?;
        let os = iter.next().ok_or(TargetParsingError::MissingOs)?;
        let abi = iter.next();
        if iter.next().is_some() {
            return Err(TargetParsingError::TooManyArguments);
        };
        let arch = Arch::from_str(arch).map_err(|_| TargetParsingError::InvalidArch)?;
        let os = Os::from_str(os).map_err(|_| TargetParsingError::InvalidOs)?;
        let abi = abi
            .map(|v| Abi::from_str(v).map_err(|_| TargetParsingError::InvalidAbi))
            .unwrap_or(Ok(Abi::None))?;
        Ok(Self { arch, os, abi })
    }
}

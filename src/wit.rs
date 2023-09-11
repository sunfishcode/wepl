use std::borrow::Cow;

use anyhow::Context;
use wit_component::DecodedWasm;
use wit_parser::{Function, Resolve, World, WorldId, WorldItem, WorldKey};

pub struct Querier {
    resolve: Resolve,
    world_id: WorldId,
}

impl Querier {
    /// Create new instance.
    ///
    /// Panics if the `world_id` is not found in the `resolve`.
    pub fn new(resolve: Resolve, world_id: WorldId) -> Self {
        let this = Self { resolve, world_id };
        // Ensure the world can be resolved
        let _ = this.world();
        this
    }

    pub fn from_bytes(component_bytes: &[u8]) -> anyhow::Result<Self> {
        let (resolve, world) = match wit_component::decode(&component_bytes)
            .context("could not decode given file as a WebAssembly component")?
        {
            DecodedWasm::Component(r, w) => (r, w),
            _ => anyhow::bail!("found wit package instead of the expect WebAssembly component"),
        };
        Ok(Self::new(resolve, world))
    }

    pub fn exported_function(&self, name: &str) -> anyhow::Result<&Function> {
        let export = self.export(name)?;
        match export {
            wit_parser::WorldItem::Function(f) => Ok(f),
            _ => anyhow::bail!("Unrecognized function '{name}'"),
        }
    }

    pub fn export(&self, name: &str) -> anyhow::Result<&WorldItem> {
        self.world()
            .exports
            .iter()
            .find_map(|(export_name, export)| {
                let WorldKey::Name(n) = export_name else {
                    return None;
                };
                (n == name).then_some(export)
            })
            .with_context(|| format!("no export with name '{name}'"))
    }

    pub fn display_wit_type<'a>(&self, param_type: &wit_parser::Type) -> Cow<'a, str> {
        let str = match param_type {
            wit_parser::Type::Bool => "bool",
            wit_parser::Type::U8 => "u8",
            wit_parser::Type::U16 => "u16",
            wit_parser::Type::U32 => "u32",
            wit_parser::Type::U64 => "u64",
            wit_parser::Type::S8 => "s8",
            wit_parser::Type::S16 => "s16",
            wit_parser::Type::S32 => "s32",
            wit_parser::Type::S64 => "s64",
            wit_parser::Type::Float32 => "float32",
            wit_parser::Type::Float64 => "float64",
            wit_parser::Type::String => "string",
            wit_parser::Type::Char => "char",
            wit_parser::Type::Id(id) => {
                let typ = self
                    .resolve
                    .types
                    .get(*id)
                    .expect("found type id for type not present in resolver");
                let name = match typ.name.clone() {
                    Some(n) => n,
                    None => match &typ.kind {
                        wit_parser::TypeDefKind::Option(o) => {
                            format!("option<{}>", self.display_wit_type(o))
                        }
                        wit_parser::TypeDefKind::Result(r) => {
                            let ok = r.ok.as_ref().map(|o| self.display_wit_type(o));
                            let err = r.err.as_ref().map(|o| self.display_wit_type(o));
                            match (ok, err) {
                                (Some(ok), Some(err)) => format!("result<{ok}, {err}>"),
                                (Some(t), _) | (_, Some(t)) => format!("result<{t}>"),
                                _ => format!("result"),
                            }
                        }
                        wit_parser::TypeDefKind::Type(t) => return self.display_wit_type(t),
                        wit_parser::TypeDefKind::Unknown => unreachable!(),
                        wit_parser::TypeDefKind::Record(_) => todo!(),
                        wit_parser::TypeDefKind::Resource => todo!(),
                        wit_parser::TypeDefKind::Handle(_) => todo!(),
                        wit_parser::TypeDefKind::Flags(_) => todo!(),
                        wit_parser::TypeDefKind::Tuple(_) => todo!(),
                        wit_parser::TypeDefKind::Variant(_) => todo!(),
                        wit_parser::TypeDefKind::Enum(_) => todo!(),
                        wit_parser::TypeDefKind::List(_) => todo!(),
                        wit_parser::TypeDefKind::Future(_) => todo!(),
                        wit_parser::TypeDefKind::Stream(_) => todo!(),
                    },
                };
                return Cow::Owned(name);
            }
        };
        Cow::Borrowed(str)
    }

    pub fn imports_wasi(&self) -> bool {
        let world = self.world();
        for (import_name, _) in &world.imports {
            if let WorldKey::Interface(interface_id) = import_name {
                let interface = self.resolve.interfaces.get(*interface_id).unwrap();
                if let Some(package_id) = &interface.package {
                    if let Some(package) = self.resolve.packages.get(*package_id) {
                        if package.name.namespace == "wasi" {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn non_wasi_imports(&self) -> impl Iterator<Item = (String, &WorldItem)> {
        self.world()
            .imports
            .iter()
            .filter_map(|(import_name, import)| {
                let import_name = match import_name {
                    WorldKey::Name(n) => n.clone(),
                    WorldKey::Interface(i) => {
                        let interface = self.resolve.interfaces.get(*i).unwrap();
                        match &interface.package {
                            Some(package_id) => {
                                let package = self.resolve.packages.get(*package_id).unwrap();
                                if package.name.namespace == "wasi" {
                                    return None;
                                }
                                format!("{}", package.name)
                            }
                            None => todo!(),
                        }
                    }
                };
                Some((import_name, import))
            })
    }

    pub fn world_item_name(&self, name: &WorldKey) -> anyhow::Result<String> {
        let import_name = match name {
            WorldKey::Name(n) => n.clone(),
            WorldKey::Interface(i) => {
                let interface = self.resolve.interfaces.get(*i).unwrap();
                match &interface.package {
                    Some(package_id) => {
                        let package = self.resolve.packages.get(*package_id).unwrap();
                        format!("{}/{}", package.name, interface.name.as_ref().unwrap())
                    }
                    None => todo!(),
                }
            }
        };
        Ok(import_name)
    }

    pub fn world(&self) -> &World {
        self.resolve
            .worlds
            .get(self.world_id)
            .expect("world_id is not found in the resolved wit package")
    }
}
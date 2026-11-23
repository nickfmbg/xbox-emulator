use super::{semantic, sig_type, Emitter};
use crate::meta::{SigElement, TessInfo};
use crate::Stage;
use std::fmt::Write as _;

pub const DS_TESS_U: &str = "v0";
pub const DS_TESS_V: &str = "v1";
pub const DS_REL_PATCH_ID: &str = "v2";
pub const DS_PATCH_ID: &str = "v3";

pub fn domain_attribute(domain: u32) -> Result<&'static str, String> {
    match domain {
        1 => Ok("isoline"),
        2 => Ok("tri"),
        3 => Ok("quad"),
        other => Err(format!("tessellator domain {other} unsupported")),
    }
}

pub fn is_tess_factor(e: &SigElement) -> bool {
    (11..=16).contains(&e.sysval)
}

pub fn patch_struct(sig: &[SigElement]) -> String {
    if sig.is_empty() {
        return String::new();
    }
    let mut h = String::from("struct SPatch {\n");
    let mut factor_names: Vec<&str> = Vec::new();
    for e in sig.iter().filter(|e| is_tess_factor(e)) {
        if !factor_names.contains(&e.name.as_str()) {
            factor_names.push(&e.name);
        }
    }
    for (k, name) in factor_names.iter().enumerate() {
        let n = sig.iter().filter(|e| is_tess_factor(e) && e.name == *name).count();
        let _ = if n == 1 {
            writeln!(h, "    float tb_tf{k} : {name};")
        } else {
            writeln!(h, "    float tb_tf{k}[{n}] : {name};")
        };
    }
    for (ei, e) in sig.iter().enumerate().filter(|(_, e)| !is_tess_factor(e)) {
        let (ty, _) = sig_type(e);
        let _ = writeln!(h, "    {ty} p{ei} : {};", semantic(e));
    }
    h.push_str("};\n");
    h
}

impl<'a> Emitter<'a> {
    pub(super) fn is_ds(&self) -> bool {
        self.ex.stage == Stage::Domain
    }

    pub(super) fn tess_info(&self) -> Result<TessInfo, String> {
        let want = if self.is_ds() { 4 } else { 3 };
        self.ex
            .gs_info
            .and_then(|g| g.tess)
            .filter(|t| t.stage == want)
            .ok_or_else(|| format!("{:?} shader without PSV0 tessellator info", self.ex.stage))
    }

    pub(super) fn ds_check(&self) -> Result<(), String> {
        self.tess_info()?;
        if self.ex.output_sig.is_empty() {
            return Err("domain shader without outputs unsupported".into());
        }
        Ok(())
    }

    pub(super) fn ds_entry(&self) -> Result<String, String> {
        let t = self.tess_info()?;
        let loc_ty = if t.domain == 2 { "float3" } else { "float2" };
        let mut params = vec![format!("{loc_ty} tb_loc : SV_DomainLocation"), "uint tb_pid : SV_PrimitiveID".to_string()];
        if !self.ex.input_sig.is_empty() {
            params.push(format!("const OutputPatch<SIn, {}> i", t.input_control_points));
        }
        let patch = patch_struct(&self.ex.prim_sig);
        if !patch.is_empty() {
            params.push("SPatch tb_pc".to_string());
        }
        Ok(format!(
            "{patch}[domain(\"{}\")]\nSOut main({}) {{\n    SOut o = (SOut)0;\n",
            domain_attribute(t.domain)?,
            params.join(", ")
        ))
    }

    pub(super) fn ds_init(&mut self) -> String {
        let mut init = String::new();
        let seeds = [
            (DS_TESS_U, "asuint(tb_loc.x)"),
            (DS_TESS_V, "asuint(tb_loc.y)"),
            (DS_REL_PATCH_ID, "tb_pid"),
            (DS_PATCH_ID, "tb_pid"),
        ];
        for (v, val) in seeds {
            if self.live.iter().any(|l| l == v) {
                self.regs.insert(v.to_string());
                let _ = writeln!(init, "    {v} = {val};");
            }
        }
        init
    }
}

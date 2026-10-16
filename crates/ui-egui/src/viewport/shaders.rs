//! The viewport's GLSL, as data. The `#version` line is not here: `super::gl` prefixes
//! the one the window's GL context reports (egui_glow's `ShaderVersion`; desktop GL 3.x
//! gives GLSL 1.40), so the sources use only `in`/`out` and no `layout` qualifiers, and
//! attributes are bound by name to the locations in [`ProgramSrc::attributes`].
//!
//! The tests parse every source for its declarations: the attributes the GL code binds,
//! the uniforms it sets, matching varyings between the stages and one color output.
//! The GPU driver compiles them for real when the viewport first draws; a failure is
//! shown in the viewport, never a panic.

/// One shader program: its two stages, the attribute names bound to locations 0, 1, ...
/// in order, and the uniforms the GL code sets.
#[derive(Debug, Clone, Copy)]
pub struct ProgramSrc {
    pub name: &'static str,
    pub vertex: &'static str,
    pub fragment: &'static str,
    pub attributes: &'static [&'static str],
    pub uniforms: &'static [&'static str],
}

/// Attribute: vertex position (location 0 in every program).
pub const A_POS: &str = "a_pos";
/// Attribute: line color (location 1 of [`LINES`]).
pub const A_COLOR: &str = "a_color";
/// Attribute: face normal (location 1 of [`SOLID`]).
pub const A_NORMAL: &str = "a_normal";

/// Uniform: world to clip space.
pub const U_VIEW_PROJ: &str = "u_view_proj";
/// Uniform: clip to world space (the sky reconstructs view rays with it).
pub const U_INV_VIEW_PROJ: &str = "u_inv_view_proj";
/// Uniform: model to world space.
pub const U_MODEL: &str = "u_model";
/// Uniform: the eye position (lines fade with distance from it).
pub const U_EYE: &str = "u_eye";
/// Uniform: distances where line fading starts and ends.
pub const U_FADE: &str = "u_fade";
/// Uniform: fill color.
pub const U_COLOR: &str = "u_color";
/// Uniform: unit direction toward the light.
pub const U_LIGHT: &str = "u_light";
/// Uniforms: the sky straight up, at the horizon, and the ground tint below it.
pub const U_ZENITH: &str = "u_zenith";
/// See [`U_ZENITH`].
pub const U_HORIZON: &str = "u_horizon";
/// See [`U_ZENITH`].
pub const U_GROUND: &str = "u_ground";

/// The horizon tint: a full-viewport triangle whose color follows the elevation of the
/// view ray through each pixel (zenith above, ground tint below the horizon).
pub const SKY: ProgramSrc = ProgramSrc {
    name: "sky",
    vertex: "
in vec2 a_pos;
out vec2 v_ndc;
void main() {
    v_ndc = a_pos;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
",
    fragment: "
uniform mat4 u_inv_view_proj;
uniform vec4 u_zenith;
uniform vec4 u_horizon;
uniform vec4 u_ground;
in vec2 v_ndc;
out vec4 o_color;
void main() {
    vec4 ray_near = u_inv_view_proj * vec4(v_ndc, -1.0, 1.0);
    vec4 ray_far = u_inv_view_proj * vec4(v_ndc, 1.0, 1.0);
    vec3 dir = normalize(ray_far.xyz / ray_far.w - ray_near.xyz / ray_near.w);
    float up = clamp(dir.z, -1.0, 1.0);
    vec3 sky = mix(u_horizon.rgb, u_zenith.rgb, sqrt(max(up, 0.0)));
    vec3 ground = mix(u_horizon.rgb, u_ground.rgb, sqrt(max(-up, 0.0)));
    o_color = vec4(up >= 0.0 ? sky : ground, 1.0);
}
",
    attributes: &[A_POS],
    uniforms: &[U_INV_VIEW_PROJ, U_ZENITH, U_HORIZON, U_GROUND],
};

/// Colored lines (grid, axes, marker outlines), faded with distance from the eye.
pub const LINES: ProgramSrc = ProgramSrc {
    name: "lines",
    vertex: "
uniform mat4 u_view_proj;
uniform mat4 u_model;
in vec3 a_pos;
in vec4 a_color;
out vec4 v_color;
out vec3 v_world;
void main() {
    vec4 world = u_model * vec4(a_pos, 1.0);
    v_world = world.xyz;
    v_color = a_color;
    gl_Position = u_view_proj * world;
}
",
    fragment: "
uniform vec3 u_eye;
uniform vec2 u_fade;
in vec4 v_color;
in vec3 v_world;
out vec4 o_color;
void main() {
    float keep = 1.0 - smoothstep(u_fade.x, u_fade.y, distance(v_world, u_eye));
    o_color = vec4(v_color.rgb, v_color.a * keep);
}
",
    attributes: &[A_POS, A_COLOR],
    uniforms: &[U_VIEW_PROJ, U_MODEL, U_EYE, U_FADE],
};

/// Flat-lit faces (marker boxes): one color, ambient plus diffuse light.
pub const SOLID: ProgramSrc = ProgramSrc {
    name: "solid",
    vertex: "
uniform mat4 u_view_proj;
uniform mat4 u_model;
in vec3 a_pos;
in vec3 a_normal;
out vec3 v_normal;
void main() {
    v_normal = mat3(u_model) * a_normal;
    gl_Position = u_view_proj * u_model * vec4(a_pos, 1.0);
}
",
    fragment: "
uniform vec4 u_color;
uniform vec3 u_light;
in vec3 v_normal;
out vec4 o_color;
void main() {
    float light = 0.35 + 0.65 * max(dot(normalize(v_normal), u_light), 0.0);
    o_color = vec4(u_color.rgb * light, u_color.a);
}
",
    attributes: &[A_POS, A_NORMAL],
    uniforms: &[U_VIEW_PROJ, U_MODEL, U_COLOR, U_LIGHT],
};

/// Every program the viewport builds.
pub const PROGRAMS: [ProgramSrc; 3] = [SKY, LINES, SOLID];

/// The text put before every stage: the context's `#version` line, and a default float
/// precision on OpenGL ES (which requires one in fragment shaders).
pub fn header(version_declaration: &str, embedded: bool) -> String {
    let mut out = version_declaration.to_string();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    if embedded {
        out.push_str("precision highp float;\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(qualifier, type, name)` of every top-level `in`/`out`/`uniform` declaration.
    fn declarations(src: &str) -> Vec<(String, String, String)> {
        src.lines()
            .filter_map(|l| {
                let l = l.trim().strip_suffix(';')?;
                let mut words = l.split_whitespace();
                let q = words.next()?;
                if !["in", "out", "uniform"].contains(&q) {
                    return None;
                }
                let (ty, name) = (words.next()?, words.next()?);
                assert!(words.next().is_none(), "one name per declaration: {l}");
                Some((q.to_string(), ty.to_string(), name.to_string()))
            })
            .collect()
    }

    fn balanced(src: &str) -> bool {
        let mut depth: i64 = 0;
        for c in src.chars() {
            match c {
                '{' | '(' => depth = depth.saturating_add(1),
                '}' | ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth < 0 {
                return false;
            }
        }
        depth == 0
    }

    #[test]
    fn every_program_declares_what_the_gl_code_binds_and_sets() {
        for p in PROGRAMS {
            for (stage, src) in [("vertex", p.vertex), ("fragment", p.fragment)] {
                assert!(src.contains("void main() {"), "{} {stage}: no main", p.name);
                assert!(balanced(src), "{} {stage}: unbalanced", p.name);
                assert!(!src.contains("#version"), "{} {stage}", p.name);
                assert!(!src.contains("layout("), "{} {stage}", p.name);
                assert!(!src.contains("gl_FragColor") && !src.contains("varying"));
            }
            let vs = declarations(p.vertex);
            let fs = declarations(p.fragment);
            // Attributes: exactly the bound names, in vertex-shader `in`s.
            let ins: Vec<&str> = vs
                .iter()
                .filter(|d| d.0 == "in")
                .map(|d| d.2.as_str())
                .collect();
            let mut want = p.attributes.to_vec();
            want.sort_unstable();
            let mut got = ins.clone();
            got.sort_unstable();
            assert_eq!(got, want, "{}: attributes", p.name);
            assert_eq!(p.attributes.first(), Some(&A_POS), "{}", p.name);
            // Uniforms: every one set is declared in a stage, and every declared one is set.
            let declared: Vec<&str> = vs
                .iter()
                .chain(&fs)
                .filter(|d| d.0 == "uniform")
                .map(|d| d.2.as_str())
                .collect();
            for u in p.uniforms {
                assert!(declared.contains(u), "{}: {u} not declared", p.name);
            }
            for u in &declared {
                assert!(
                    p.uniforms.contains(u),
                    "{}: {u} declared, never set",
                    p.name
                );
            }
            // Varyings: each vertex `out` is a fragment `in` of the same type, and back.
            let vs_out: Vec<_> = vs.iter().filter(|d| d.0 == "out").collect();
            let fs_in: Vec<_> = fs.iter().filter(|d| d.0 == "in").collect();
            assert_eq!(vs_out.len(), fs_in.len(), "{}: varyings", p.name);
            for v in &vs_out {
                assert!(
                    fs_in.iter().any(|f| f.1 == v.1 && f.2 == v.2),
                    "{}: varying {v:?} unmatched",
                    p.name
                );
            }
            // One color output.
            let fs_out: Vec<_> = fs.iter().filter(|d| d.0 == "out").collect();
            assert_eq!(fs_out.len(), 1, "{}", p.name);
            assert_eq!(fs_out[0].1, "vec4", "{}", p.name);
        }
    }

    #[test]
    fn the_header_carries_the_version_and_es_precision() {
        assert_eq!(header("#version 140\n", false), "#version 140\n");
        assert_eq!(
            header("#version 300 es", true),
            "#version 300 es\nprecision highp float;\n"
        );
    }
}

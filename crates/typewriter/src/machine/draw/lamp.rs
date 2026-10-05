//! The lamp through the render crate: the frame's [`light`] turned into the
//! uniform the shader reads, and the shadow pass's own rows.
//!
//! T3 lands the lit materials and shadow reading in `typewriter-render`'s
//! `scene.wgsl`; this module is the app's half — the same CPU lighting the
//! old pass uses, laid out for the new pipeline. Keep in step with
//! `depth::lighting` and `scene.wgsl`.

use typewriter_render::device::LightingUniform;

use super::super::light;

/// The frame's lighting, as the new pipeline's uniform reads it.
pub(crate) fn uniform() -> LightingUniform {
    let lighting = light::frame();
    let mut bands = [glam::Vec4::ZERO; 5];
    for (band, &(down, colour)) in bands.iter_mut().zip(&lighting.chrome) {
        let [r, g, b, _] = colour.to_normalized_gamma_f32();
        *band = glam::Vec4::new(down, r, g, b);
    }
    LightingUniform {
        lamp: lighting.lamp.extend(lighting.lamp_radius),
        eye: lighting.eye.extend(lighting.falloff),
        bands,
        shadow: lighting.shadow.rows,
        params: glam::Vec4::new(
            lighting.shadow.bias,
            lighting.shadow.side,
            lighting.shadow.penumbra,
            lighting.shadow.span,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The uniform is the frame's light in the shader's own words: the lamp
    /// and the eye first, the chrome bands, then the shadow map's rows and
    /// their parameters — the very order `depth::lighting::Lighting::uniform`
    /// packs the old pass's `r_lighting`, so the new pass lights what the old
    /// one did.
    #[test]
    fn the_uniform_is_the_frames_light_in_the_shaders_words() {
        let lighting = light::frame();
        let shadow = light::shadow();
        let uniform = uniform();
        assert_eq!(typewriter_render::device::LIGHTING_BYTES, 11 * 16);

        // [0] the lamp's head, `w` its radius; [1] toward the eye, `w` the
        // falloff's reference distance.
        assert_eq!(uniform.lamp, lighting.lamp.extend(lighting.lamp_radius));
        assert_eq!(uniform.lamp.truncate(), light::lamp());
        assert_eq!(uniform.lamp.w, lighting.lamp_radius);
        assert_eq!(uniform.eye, lighting.eye.extend(lighting.falloff));
        assert_eq!(uniform.eye.truncate(), lighting.eye);
        assert_eq!(uniform.eye.w, lighting.falloff);

        // [2..] chrome's bands, `x` how far down and `yzw` its colour in
        // gamma, as `Lighting::uniform` packs them.
        assert_eq!(uniform.bands.len(), lighting.chrome.len());
        for (band, &(down, colour)) in uniform.bands.iter().zip(&lighting.chrome) {
            let [r, g, b, _] = colour.to_normalized_gamma_f32();
            assert_eq!(*band, glam::Vec4::new(down, r, g, b));
        }

        // [7..9] the shadow map's rows and [10] its parameters: bias, side,
        // the penumbra's scale and the map's depth run.
        assert_eq!(uniform.shadow, shadow.rows);
        assert_eq!(uniform.shadow, lighting.shadow.rows);
        assert_eq!(
            uniform.params,
            glam::Vec4::new(shadow.bias, shadow.side, shadow.penumbra, shadow.span)
        );
    }

    /// The words ride the bytes in the shader's order, so a later field swap
    /// cannot pass unnoticed.
    #[test]
    fn the_uniform_words_are_in_the_shaders_order() {
        let lighting = light::frame();
        let shadow = light::shadow();
        let uniform = uniform();
        let bytes = bytemuck::bytes_of(&uniform);
        let word = |k: usize| {
            let at = k * 16;
            std::array::from_fn(|i| {
                f32::from_le_bytes(bytes[at + i * 4..at + i * 4 + 4].try_into().unwrap())
            })
        };
        assert_eq!(
            word(0),
            lighting.lamp.extend(lighting.lamp_radius).to_array()
        );
        assert_eq!(word(1), lighting.eye.extend(lighting.falloff).to_array());
        assert_eq!(word(7), shadow.rows[0].to_array());
        assert_eq!(word(8), shadow.rows[1].to_array());
        assert_eq!(word(9), shadow.rows[2].to_array());
        assert_eq!(
            word(10),
            [shadow.bias, shadow.side, shadow.penumbra, shadow.span]
        );
    }
}

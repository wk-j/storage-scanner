//! Fire compositor adapted from ui-lab/src/bin/fx/gfx.rs (Rune showreel, MIT, Anomaly).
//! Uses the same rune-fx simulated fire, only while a confirmed clear is animating.
use rune::compose::{Compositor, DeviceKey, Frame};
use rune::render::{EffectPhase, EffectSite, KIND_EFFECT};
use rune_fx::*;
use crate::model::State;

pub const FIRE: u32 = 4;
#[derive(Default)]
pub struct Gfx {
    fx: Option<(DeviceKey, FxRenderer)>,
    burning: bool,
}
impl Compositor<State> for Gfx {
    fn animating(&self) -> bool { self.burning }
    fn render(&mut self, f: &mut Frame<'_, State>) {
        let (device, queue, size) = (f.device, f.queue, f.size);
        let mut at = Vec::new();
        let mut layers = Vec::new();
        let mut placements = Vec::new();
        for (n, i) in f.rt.instances.iter().enumerate() {
            if i.params[1] != KIND_EFFECT || f.rt.marks.get(n).copied().unwrap_or(0) != FIRE { continue; }
            let k = i.params[3];
            let [x, y, w, h] = i.rect.map(|v| v / k);
            let [height, intensity, smoke, _] = i.uv;
            if intensity <= 0.0 { continue; }
            at.push(n as u32);
            layers.push(Fx::fire(FireParams {
                style: FireStyle::Top, height, intensity, smoke,
                embers: 24, scale: 26.0, ..FireParams::default()
            }).on(Shape::rounded(x, y, w, h, 2.0)).at(f.rt.t as f32));
            placements.push(FxSlot::default().scale(k).clip(i.clip, i.fade));
        }
        self.burning = !layers.is_empty();
        let key = f.device_key();
        if self.fx.as_ref().is_none_or(|(k, _)| *k != key) {
            let mut fx = FxRenderer::new(device, f.format, FxOptions { dither: true });
            fx.set_fire_sim(device, true);
            self.fx = Some((key, fx));
        }
        let fx = &mut self.fx.as_mut().unwrap().1;
        fx.set_srgb_blend(f.srgb_blend());
        fx.add_format(device, wgpu::TextureFormat::Rgba16Float);
        fx.prepare_with_slots(device, queue, &FxFrame { viewport: [size.0, size.1], scale: 1.0, layers: &layers, backdrop: None, sources: &[] }, &placements);
        if self.burning { fx.simulate(queue, f.encoder, None); }
        let clear = f.rt.clear_color();
        f.ui.prepare(device, queue, &mut f.rt.text.atlas, &f.rt.instances, size);
        f.ui.draw_with_backdrop(f.encoder, f.target, clear, &mut |phase, site: &EffectSite<'_>| {
            let Some(k) = at.iter().position(|n| *n == site.index) else { return };
            if let EffectPhase::Draw(pass) = phase { fx.draw_layer_in(pass, site.format, site.origin, k); }
        });
    }
}

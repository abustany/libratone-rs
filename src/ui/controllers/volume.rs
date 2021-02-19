use druid::{Env, UpdateCtx, Widget};
use druid::widget::Controller;

use super::super::appstate::Device;
use super::super::commands::SetVolume;

pub struct VolumeController;

impl<W: Widget<Device>> Controller<Device, W> for VolumeController {
    fn update(&mut self, child: &mut W, ctx: &mut UpdateCtx, old_data: &Device, data: &Device, env: &Env) {
        if old_data.volume != data.volume && data.volume.is_some() {
            ctx.submit_command(SetVolume::new(&data.id, data.volume.unwrap()));
        }

        child.update(ctx, old_data, data, env);
    }
}

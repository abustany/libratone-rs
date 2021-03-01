use druid::{Env, UpdateCtx, Widget};
use druid::widget::Controller;

use super::super::appstate::Device;
use super::super::commands::SendCommand;
use crate::commands::{Command, Volume};

pub struct VolumeController;

impl<W: Widget<Device>> Controller<Device, W> for VolumeController {
    fn update(&mut self, child: &mut W, ctx: &mut UpdateCtx, old_data: &Device, data: &Device, env: &Env) {
        if old_data.volume != data.volume && data.volume.is_some() {
            let volume = data.volume.unwrap();
            ctx.submit_command(SendCommand::new(
                &data.id,
                Volume::set(volume),
                Box::new(move |d: &mut Device| d.volume = Some(volume)),
            ));
        }

        child.update(ctx, old_data, data, env);
    }
}

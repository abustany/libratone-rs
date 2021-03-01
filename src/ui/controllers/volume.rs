use druid::widget::Controller;
use druid::{Env, UpdateCtx, Widget};

use crate::commands::{Command, Volume};
use crate::ui::appstate::Device;
use crate::ui::commands::SendCommand;

pub struct VolumeController;

impl<W: Widget<Device>> Controller<Device, W> for VolumeController {
    fn update(
        &mut self,
        child: &mut W,
        ctx: &mut UpdateCtx,
        old_data: &Device,
        data: &Device,
        env: &Env,
    ) {
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

use druid::{Data, Widget, WidgetExt};
use druid::widget::{Flex, Label, MainAxisAlignment, Padding};

pub struct Page;

impl Page {
    pub fn new<T: Data, F>(
        title: F,
        content: impl Widget<T> + 'static,
        back_button: Option<Box<dyn Widget<T>>>,
    ) -> impl Widget<T> + 'static
    where F: Fn(&T) -> String + 'static
    {
        let mut header_row = Flex::row()
            .must_fill_main_axis(true)
            .main_axis_alignment(MainAxisAlignment::Center);

        if let Some(button) = back_button {
            header_row.add_child(button);
        }

        header_row.add_flex_child(
            Label::new(move |data: &T, _env: &_| title(data)).expand_width(),
            1.0,
        );

        let header = Padding::new(10.0, header_row);

        Flex::column()
            .with_child(header)
            .with_flex_child(content, 1.0)
    }
}

pub struct ListBtn {
    id: Option<nuon::Id>,
    size: nuon::Size,
    color: nuon::Color,
    label: String,
    font_size: f32,
    is_selected: bool,
}

impl ListBtn {
    pub fn new() -> Self {
        Self {
            id: None,
            size: Default::default(),
            color: nuon::Color::WHITE,
            label: Default::default(),
            font_size: 14.0,
            is_selected: false,
        }
    }

    #[allow(unused)]
    pub fn id(mut self, id: impl Into<nuon::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = (width, height).into();
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn color(mut self, color: impl Into<nuon::Color>) -> Self {
        self.color = color.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.is_selected = selected;
        self
    }

    pub fn build(&self, ui: &mut nuon::Ui) -> bool {
        let w = self.size.width;
        let h = self.size.height;

        let id = if let Some(id) = self.id {
            id
        } else {
            nuon::Id::hash(&self.label)
        };

        let event = nuon::click_area(id).size(w, h).build(ui);

        let (bg, border_color) = if self.is_selected {
            (
                nuon::Color::new_u8(50, 45, 10, 0.9), // Muted dark golden background for selected
                nuon::Color::new_u8(255, 215, 0, 1.0), // Bright yellow accent border
            )
        } else if event.is_hovered() || event.is_pressed() {
            (
                nuon::Color::new_u8(35, 35, 45, 0.8), // Hover state background
                nuon::Color::new_u8(100, 100, 120, 1.0),
            )
        } else {
            (
                nuon::Color::new_u8(20, 20, 25, 0.7), // Clean dark list item background
                nuon::Color::new_u8(40, 40, 50, 1.0),
            )
        };

        // Background box
        nuon::quad()
            .size(w, h)
            .color(bg)
            .border_radius([4.0; 4])
            .build(ui);

        // Left accent indicator for selected item
        if self.is_selected {
            nuon::quad()
                .size(4.0, h)
                .color(border_color)
                .border_radius([4.0, 0.0, 0.0, 4.0])
                .build(ui);
        }

        // Label text
        let text_color = if self.is_selected {
            nuon::Color::new_u8(255, 230, 80, 1.0) // Vibrant yellow text when selected
        } else {
            self.color // Crisp light-gray/white text when unselected
        };

        nuon::label()
            .size(self.size.width, self.size.height)
            .font_size(self.font_size)
            .color(text_color)
            .text(&self.label)
            .build(ui);

        event.is_clicked()
    }
}

pub fn list_btn() -> ListBtn {
    ListBtn::new()
}

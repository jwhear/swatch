use adobe_swatch_exchange::{create_ase, read_ase, ColorBlock, ColorType, ColorValue, Group};
use eframe::egui;
use egui::Color32;
use rfd::FileDialog;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).map(|s| PathBuf::from(s));

    let options = eframe::NativeOptions {
        multisampling: 8,
        ..Default::default()
    };
    eframe::run_native(
        "Swatch",
        options,
        Box::new(|cc| {
            let app = App::new(cc, path);
            Ok(Box::new(app))
        }),
    )
}

struct App {
    // Where we'll save to. This is set by Load and Save As
    save_path: Option<std::path::PathBuf>,
    // An ASE has groups of colors
    groups: Vec<Group>,
    // And ungrouped colors
    ungrouped: Vec<ColorBlock>,
    // If we experience a recoverable error, push it here and we'll display it to the user
    errors: Vec<String>,
}

impl App {
    fn new(cc: &eframe::CreationContext, path: Option<PathBuf>) -> Self {
        cc.egui_ctx.set_zoom_factor(1.3);

        let mut ret = Self {
            save_path: path,
            groups: Vec::new(),
            ungrouped: Vec::new(),
            errors: Vec::new(),
        };

        // If called with an initial path (e.g. the user ran `swatch something.ase`),
        //  load it
        if let Some(path) = &ret.save_path {
            match load_from_path(&path) {
                Ok((groups, ungrouped)) => {
                    ret.groups = groups;
                    ret.ungrouped = ungrouped;
                }
                Err(e) => {
                    ret.errors.push(format!("{e}"));
                }
            }
        }

        ret
    }

    fn set_save_path(&mut self) {
        // Prompt for a path
        let mut dlg = FileDialog::new()
            .add_filter("Adobe Swatch Exchange", &["ase"])
            .set_file_name("colors.ase");

        // Try to initialize the Save As window:
        // 1) on the current save path if there is one
        // 2) on the current directory if there is one
        // 3) whatever default it has
        let dir = self
            .save_path
            .as_ref()
            .and_then(|p| p.parent().map(|v| v.to_path_buf()))
            .or_else(|| std::env::current_dir().ok());
        if let Some(path) = dir {
            dlg = dlg.set_directory(path);
        };

        self.save_path = dlg.save_file();
    }

    fn save(&mut self) {
        if self.save_path.is_none() {
            self.set_save_path();
        }

        // If no path then we don't save. This can happen if the user clicks
        //  "Cancel" on the save dialog.
        if let Some(path) = &self.save_path {
            let bytes = create_ase(self.groups.clone(), self.ungrouped.clone());
            if let Err(e) = std::fs::write(path, bytes) {
                self.errors.push(format!("{e}"));
            }
        }
    }

    fn open(&mut self) {
        fn inner(app: &mut App) -> Result<()> {
            let mut dlg = FileDialog::new().add_filter("Adobe Swatch Exchange", &["ase"]);
            if let Ok(cwd) = std::env::current_dir() {
                dlg = dlg.set_directory(cwd);
            }

            if let Some(path) = dlg.pick_file() {
                (app.groups, app.ungrouped) = load_from_path(&path)?;
                // remember the path so that save overwrites the existing file
                app.save_path = Some(path);
            }
            Ok(())
        }

        if let Err(e) = inner(self) {
            self.errors.push(format!("{e}"));
        }
    }
}

fn load_from_path(path: &Path) -> Result<(Vec<Group>, Vec<ColorBlock>)> {
    let bytes = std::fs::read(path)?;
    read_ase(&*bytes).map_err(|e| e.into())
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Main menu
        egui::TopBottomPanel::top("main_menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        self.open();
                        ui.close_menu();
                    }

                    if ui.button("Save").clicked() {
                        self.save();
                        ui.close_menu();
                    }

                    if ui.button("Save As").clicked() {
                        self.set_save_path();
                        if self.save_path.is_some() {
                            self.save();
                        }
                        ui.close_menu();
                    }

                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                })
            });
        });

        // Show errors at the bottom
        egui::TopBottomPanel::bottom("errors").show(ctx, |ui| {
            for error in self.errors.iter() {
                ui.colored_label(Color32::RED, error);
            }
        });

        // Since we want to render both groups and ungrouped, define a helper
        //  that can render the UI for a Vec<ColorBlock>
        let render_vec_of_color = |ui: &mut egui::Ui, vec: &mut Vec<ColorBlock>| {
            for block in vec.iter_mut() {
                ui.add_sized((108.0, 130.0), color_block(block));
            }

            // Show a "New" button to add a color to this vec
            let resp = ui.add_sized(
                egui::Vec2 { x: 100.0, y: 100.0 },
                egui::Button::new("Add New"),
            );

            // If the button is clicked, push a new color onto the vec
            if resp.clicked() {
                vec.push(ColorBlock::new(
                    "new".to_string(),
                    ColorValue::Rgb(1., 1., 1.),
                    ColorType::Normal,
                ));
            }
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // Render each group as a collapsible frame
                    for group in self.groups.iter_mut() {
                        ui.collapsing(&group.name, |ui| {
                            render_vec_of_color(ui, &mut group.blocks);
                        });
                    }
                    // Render all ungrouped
                    render_vec_of_color(ui, &mut self.ungrouped);
                });
            });
        });
    }
}

// A block representing a color in the swatch.
// We render a large rectangle filled with the color, a picker button, and the name
// Clicking on the large rectangle causes the color's hex value to be put on the clipboard
fn color_block(block: &mut ColorBlock) -> impl FnMut(&mut egui::Ui) -> egui::Response + '_ {
    move |ui| {
        // egui uses its Color32 type while the ASE library has its own color enumeration
        // We need to translate between them
        use egui::Color32 as C;
        let mut as_color32 = match &block.color {
            ColorValue::Rgb(r, g, b) => {
                C::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
            }
            //TODO could render a "broken" state with appropriate hover_text
            // For now let's just crash out
            other => panic!("unsupported color type: {other:?}"),
        };

        let resp = ui
            .group(|ui| {
                ui.vertical(|ui| {
                    let (rect, response) = ui.allocate_exact_size(
                        egui::Vec2 { x: 100.0, y: 100.0 },
                        egui::Sense::click(),
                    );

                    // When clicked, copy the hex code to the clipboard
                    if response.clicked() {
                        ui.output_mut(|o| o.copied_text = as_color32.to_hex());
                    }

                    ui.painter().rect_filled(rect, 2.0, as_color32);
                    ui.horizontal(|ui| {
                        ui.color_edit_button_srgba(&mut as_color32);
                        ui.text_edit_singleline(&mut block.name);
                    });
                });
            })
            .response;

        // Convert from Color32 to the ASE Rgb format
        let [r, g, b, _a] = as_color32.to_normalized_gamma_f32();
        block.color = ColorValue::Rgb(r, g, b);
        resp
    }
}

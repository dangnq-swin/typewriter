use eframe::egui;

pub struct TypewriterApp;

impl eframe::App for TypewriterApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |_ui| {});
    }
}

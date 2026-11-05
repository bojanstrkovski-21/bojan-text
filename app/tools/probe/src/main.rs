struct A(u32);
impl eframe::App for A {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _f: &mut eframe::Frame) {
        self.0 += 1;
        ui.label("hi");
        ui.ctx().request_repaint();
        if self.0 > 30 { ui.ctx().send_viewport_cmd(eframe::egui::ViewportCommand::Close); }
    }
}
fn main() {
    eprintln!("start");
    let r = eframe::run_native("p", Default::default(), Box::new(|_| Ok(Box::new(A(0)))));
    eprintln!("end {:?}", r.is_ok());
}

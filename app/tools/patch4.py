# One-off patch: a --demo-typing switch for --screenshot runs, to test Enter/Tab for real.
import io

p = 'src/app.rs'
s = io.open(p, encoding='utf-8').read()
old = "        self.sync_clones();\n        self.sync_groups();"
new = '''        // --demo-typing (screenshot runs only): type into a new file to test Enter / Tab
        if self.shot.is_some() && std::env::args().any(|a| a == "--demo-typing") {
            let key = |k: Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
            match self.frame {
                10 => {
                    self.run("new");
                    if let Some(d) = self.docs.get_mut(self.active) {
                        d.syntax_override = Some("Rust".into());
                    }
                }
                14 => self.pending_events.push(egui::Event::Text("fn main() {}".into())),
                18 => self.pending_events.push(key(Key::ArrowLeft)),
                22 => self.pending_events.push(key(Key::Enter)),
                26 => self.pending_events.push(egui::Event::Text("let x = 1;".into())),
                30 => self.pending_events.push(key(Key::Enter)),
                34 => self.pending_events.push(egui::Event::Text("let ab".into())),
                38 => self.pending_events.push(key(Key::Enter)),
                42 => self.pending_events.push(egui::Event::Text("ab".into())),
                46 => self.pending_events.push(key(Key::Tab)),
                _ => {}
            }
        }
        self.sync_clones();
        self.sync_groups();'''
assert old in s
s = s.replace(old, new, 1)
io.open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched4')

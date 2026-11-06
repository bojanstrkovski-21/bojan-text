# One-off patch: --demo-find switch (screenshot runs) to test Find in Files output.
import io
p = 'src/app.rs'
s = io.open(p, encoding='utf-8').read()
old = "        self.sync_clones();\n        self.sync_groups();"
new = '''        if self.shot.is_some() && self.frame == 12 && std::env::args().any(|a| a == "--demo-find") {
            self.folder = std::env::current_dir().ok().map(|d| d.join("src"));
            self.find.query = "fn newline".into();
            self.find.open = true;
            self.fif.open = true;
            self.fif.where_ = "<open folders>".into();
            self.start_find_in_files();
        }
        self.sync_clones();
        self.sync_groups();'''
assert old in s
s = s.replace(old, new, 1)
io.open(p, 'w', encoding='utf-8', newline='\n').write(s)
print('patched6')

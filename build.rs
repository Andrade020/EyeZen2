fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/eyezen.ico");
        res.set("FileDescription", "EyeZen - Filtros de Conforto Visual");
        res.set("ProductName", "EyeZen");
        res.set("LegalCopyright", "Copyright 2024");
        res.set("FileVersion", "2.1.0.0");
        res.set("ProductVersion", "2.1.0.0");
        res.compile().expect("winres compile");
    }
}

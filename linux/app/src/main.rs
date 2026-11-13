use gtk::prelude::*;
use webkit2gtk::WebViewExt;

fn main() {
    gtk::init().expect("GTK");
    let win = gtk::Window::new(gtk::WindowType::Toplevel);
    win.set_title("Côn Bằng TV");
    win.set_default_size(1000, 640);
    let web = webkit2gtk::WebView::new();
    web.load_html(
        &format!(
            "<body style='background:#0E1014;color:#F2F4F8;font:24px sans-serif'><h1>Côn Bằng TV {}</h1>\
             <p>GTK {}.{}.{}</p><p id=ua></p><script>ua.textContent=navigator.userAgent</script></body>",
            cbtv::version(),
            gtk::major_version(),
            gtk::minor_version(),
            gtk::micro_version(),
        ),
        None,
    );
    win.add(&web);
    win.connect_delete_event(|_, _| {
        gtk::main_quit();
        glib::Propagation::Proceed
    });
    win.show_all();
    gtk::main();
}

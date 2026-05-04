use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager, Runtime,
};

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let show = MenuItemBuilder::with_id("show", "Show Window").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("FocusBridge");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    let _tray = tray
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

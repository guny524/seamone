mod api;
mod app;
mod capture;
mod onboarding;
mod quick_review;
mod spectrum;
mod timeline;
mod workspace;

use app::App;
use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

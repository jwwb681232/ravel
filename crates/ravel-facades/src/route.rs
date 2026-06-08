use axum::Router;

pub struct Route;

impl Route {
    fn instance() -> &'static Self {
        static INSTANCE: std::sync::OnceLock<Route> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| Route)
    }

    pub fn get(_path: &str, _handler: impl axum::handler::Handler<(), ()>) -> &'static Self {
        Self::instance()
    }
    pub fn post(_path: &str, _handler: impl axum::handler::Handler<(), ()>) -> &'static Self {
        Self::instance()
    }
    pub fn put(_path: &str, _handler: impl axum::handler::Handler<(), ()>) -> &'static Self {
        Self::instance()
    }
    pub fn delete(_path: &str, _handler: impl axum::handler::Handler<(), ()>) -> &'static Self {
        Self::instance()
    }
    pub fn patch(_path: &str, _handler: impl axum::handler::Handler<(), ()>) -> &'static Self {
        Self::instance()
    }
    pub fn group(_prefix: &str, _f: impl FnOnce()) -> &'static Self {
        Self::instance()
    }
    pub fn middleware<F>(_f: F) -> &'static Self
    where
        F: Clone + Send + Sync + 'static,
    {
        Self::instance()
    }
    pub fn build() -> Router {
        unimplemented!()
    }
}

# Changelog

All notable changes to Ravel will be documented in this file.

## [0.2.0] - Unreleased

### Added
- **P1 Security**: Structured logging (tracing), CSRF protection, rate limiting,
  bcrypt password hashing, AES-256-GCM encryption
- **P2 HTTP Layer**: Session management (encrypted cookie), CookieJar/SetCookie,
  UploadedFile multipart extractor, request lifecycle (request ID, timer, hooks)
- **P3 Auth**: Auth facade (login/logout/check/guest), JWT (HS256),
  AuthGuard middleware
- **P5 DB**: Schema Builder — programmatic table creation API
- **P4 CLI**: `ravel make:job` command, job scaffolding template
- **P6 Testing**: Extended TestClient (put_json, patch_json, delete, patch),
  TestResponse assertions (assert_dont_see, assert_exact, assert_unauthorized, etc.)
- **P7**: CHANGELOG.md, CI with code coverage

### Changed
- `println!` / `eprintln!` replaced with `tracing` macros across runtime crates
- `Application::boot()` now initializes structured logging
- `Validated<T>` now rejects with `RavelError` instead of `FormRequestRejection`
- `FormRequestRejection` deprecated in favor of `RavelError`

## [0.1.0] - 2026-06-08

### Added
- Initial framework with CLI, DI container, config/ENV, HTTP routing, middleware
- Service Provider mechanism (register/boot lifecycle)
- Route Builder with fluent API (group, middleware, controller)
- FormRequest validation (Required, Min, Max, Email, Regex, In)
- SeaORM integration with ConnectionManager and migration runner
- Code scaffolding (make:controller, make:model, etc.)
- Job Queue with in-memory driver and retry support
- ravel-macros: #[derive(Job)] proc-macro
- Database abstraction: ravel-db-core + ravel-db-seaorm
- ServerBuilder with graceful shutdown
- RavelError unified error handling with IntoResponse
- Tera template engine for code generation
- Config env override (APP_* environment variables)

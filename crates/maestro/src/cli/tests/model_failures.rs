//! Registration constraint failures are refused; other database errors fail.

use crate::{cli::model::registration_failure, failure::Failure};
use maestro_kernel::{
    model::{Error as ModelError, ModelCardRegistrationError},
    store::Error as StoreError,
};
use rusqlite::{Error as SqliteError, ErrorCode, ffi};

/// A registration that the kernel database stopped with `code`.
fn stopped_by(code: ErrorCode, extended_code: i32) -> ModelCardRegistrationError {
    let sqlite = ffi::Error {
        code,
        extended_code,
    };
    ModelCardRegistrationError::Model(ModelError::Store(StoreError::Sqlite(
        SqliteError::SqliteFailure(sqlite, None),
    )))
}

#[test]
fn a_constraint_violation_refuses_the_registration() {
    let failure = registration_failure(&stopped_by(
        ErrorCode::ConstraintViolation,
        ffi::SQLITE_CONSTRAINT_TRIGGER,
    ));
    assert!(matches!(failure, Failure::Refused(_)), "{failure:?}");
}

#[test]
fn another_database_error_fails_the_registration() {
    let failure = registration_failure(&stopped_by(ErrorCode::DatabaseBusy, ffi::SQLITE_BUSY));
    assert!(matches!(failure, Failure::Failed(_)), "{failure:?}");
}

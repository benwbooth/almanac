//! A list model that is only a number of rows (the message list read a page
//! at a time: each row's message is looked up by its index). Rows are added
//! and taken away as ranges, so a list of a hundred thousand changes length
//! at no cost, and the list view keeps the rows on screen (made once) instead
//! of making them all again.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!(<QtCore/QAbstractListModel>);
        type QAbstractListModel;
        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[base = QAbstractListModel]
        type RowCount = super::RowCountRust;
    }

    extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(self: Pin<&mut RowCount>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut RowCount>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(self: Pin<&mut RowCount>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut RowCount>);
    }

    unsafe extern "RustQt" {
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &RowCount, parent: &QModelIndex) -> i32;
        #[cxx_override]
        fn data(self: &RowCount, index: &QModelIndex, role: i32) -> QVariant;
    }

    extern "RustQt" {
        /// How many rows there are.
        #[qinvokable]
        fn count(self: &RowCount) -> i32;
        /// `n` rows more at `at` (new mail at the top: 0).
        #[qinvokable]
        #[cxx_name = "insertAt"]
        fn insert_at(self: Pin<&mut RowCount>, at: i32, n: i32);
        /// `n` rows from `at` taken away.
        #[qinvokable]
        #[cxx_name = "removeAt"]
        fn remove_at(self: Pin<&mut RowCount>, at: i32, n: i32);
        /// Made `n` rows long, at the end (rows above it kept).
        #[qinvokable]
        #[cxx_name = "setCount"]
        fn set_count(self: Pin<&mut RowCount>, n: i32);
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::{QModelIndex, QVariant};

#[derive(Default)]
pub struct RowCountRust {
    n: i32,
}

impl qobject::RowCount {
    fn row_count(&self, parent: &QModelIndex) -> i32 {
        if parent.is_valid() {
            0
        } else {
            self.n
        }
    }

    fn data(&self, _index: &QModelIndex, _role: i32) -> QVariant {
        QVariant::default()
    }

    fn count(&self) -> i32 {
        self.n
    }

    fn insert_at(mut self: Pin<&mut Self>, at: i32, n: i32) {
        if n <= 0 {
            return;
        }
        let at = at.clamp(0, self.n);
        let root = QModelIndex::default();
        unsafe { self.as_mut().begin_insert_rows(&root, at, at + n - 1) };
        self.as_mut().rust_mut().n += n;
        unsafe { self.as_mut().end_insert_rows() };
    }

    fn remove_at(mut self: Pin<&mut Self>, at: i32, n: i32) {
        let at = at.clamp(0, self.n);
        let n = n.min(self.n - at);
        if n <= 0 {
            return;
        }
        let root = QModelIndex::default();
        unsafe { self.as_mut().begin_remove_rows(&root, at, at + n - 1) };
        self.as_mut().rust_mut().n -= n;
        unsafe { self.as_mut().end_remove_rows() };
    }

    fn set_count(self: Pin<&mut Self>, n: i32) {
        let had = self.n;
        let n = n.max(0);
        if n > had {
            self.insert_at(had, n - had);
        } else if n < had {
            self.remove_at(n, had - n);
        }
    }
}

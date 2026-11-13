use std::{
    alloc::{GlobalAlloc, Layout, System},
    borrow::Cow,
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use u18n::RenderError;
use u18n_showcase::{catalogs, m_app_stitle};

struct CountingAllocator;

static TRACK_ALLOCATIONS: AtomicBool = AtomicBool::new(false);
static ALLOCATION_COUNT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Every allocator operation delegates its original pointer and layout to `System`.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: The original layout is forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: The original layout is forwarded unchanged to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: The pointer and layout came from the matching system allocator call.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if TRACK_ALLOCATIONS.load(Ordering::Relaxed) {
            ALLOCATION_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: The pointer and old layout came from the system allocator; size is forwarded.
        unsafe { System.realloc(pointer, layout, size) }
    }
}

struct AllocationSession;

impl AllocationSession {
    fn begin() -> Self {
        ALLOCATION_COUNT.store(0, Ordering::Relaxed);
        TRACK_ALLOCATIONS.store(true, Ordering::Relaxed);
        Self
    }

    fn finish(self) -> usize {
        TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
        ALLOCATION_COUNT.load(Ordering::Relaxed)
    }
}

impl Drop for AllocationSession {
    fn drop(&mut self) {
        TRACK_ALLOCATIONS.store(false, Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn generated_key_and_dynamic_calls_borrow_without_allocating() {
    let locale = &catalogs::locale_en::LOCALE;
    let catalog = &catalogs::locale_en::CATALOG;
    let french_locale = &catalogs::locale_fr::LOCALE;
    let french_catalog = &catalogs::locale_fr::CATALOG;
    let tracking = AllocationSession::begin();
    let keyed: Result<Cow<'static, str>, RenderError> = black_box(m_app_stitle! { locale: locale });
    let named: Result<Cow<'static, str>, RenderError> =
        black_box(m_app_stitle! { locale: locale, args: {} });
    let keyed_fallback: Result<Cow<'static, str>, RenderError> =
        black_box(m_app_stitle! { locale: french_locale });
    let direct: Result<&'static str, RenderError> =
        black_box(catalogs::__MESSAGE_DATA_1.render_static(locale));
    let dynamic = black_box(catalog.render(1, &[]));
    let dynamic_fallback = black_box(french_catalog.render(1, &[]));
    let allocation_count = tracking.finish();

    assert_eq!(allocation_count, 0);
    assert_eq!(keyed, Ok(Cow::Borrowed("Tiny Cart")));
    assert_eq!(named, Ok(Cow::Borrowed("Tiny Cart")));
    assert_eq!(keyed_fallback, Ok(Cow::Borrowed("Tiny Cart")));
    assert_eq!(direct, Ok("Tiny Cart"));
    assert!(matches!(dynamic, Ok(Cow::Borrowed("Tiny Cart"))));
    assert!(matches!(dynamic_fallback, Ok(Cow::Borrowed("Tiny Cart"))));
}

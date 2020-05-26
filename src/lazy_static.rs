#[macro_export] macro_rules! lazy_static {
    ($($vis:vis)? static ref $name:ident : $T:ty = $e:expr;) => {
        #[allow(non_camel_case_types)] $($vis)? struct $name {}
        #[allow(non_upper_case_globals)] $($vis)? static $name : $name = $name{};
        impl std::ops::Deref for $name {
            type Target = $T;
            fn deref(&self) -> &Self::Target {
                #[allow(non_upper_case_globals)] static mut value : std::mem::MaybeUninit::<$T> = std::mem::MaybeUninit::<$T>::uninit();
                static INIT: std::sync::Once = std::sync::Once::new();
                unsafe{
                    INIT.call_once(|| { value.write($e); });
                    &value.get_ref()
                }
            }
        }
    };
}

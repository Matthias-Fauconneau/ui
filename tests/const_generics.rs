/*#![allow(incomplete_features)]#![feature(const_generics)]

// Works

#[derive(PartialEq, Eq)] struct GenericOneFieldStruct<T>{x:T}
struct ConstGenericOneFieldStruct<const M:GenericOneFieldStruct<u32>> {}

struct ConstTwoParameters<const Mx:u32,const My:u32> {}

//expected `ByRef { alloc: Allocation { bytes: [0, 0, 0, 0, 0, 0, 0, 0], relocations: Relocations(SortedMap { data: [] }), undef_mask: UndefMask { blocks: [255], len: Size { raw: 8 } }, size: Size { raw: 8 }, align: Align { pow2: 2 }, mutability: Not, extra: () }, offset: Size { raw: 0 } } : (u32, u32)`,
//      found `ByRef { alloc: Allocation { bytes: [0, 0, 0, 0, 0, 0, 0, 0], relocations: Relocations(SortedMap { data: [] }), undef_mask: UndefMask { blocks: [255], len: Size { raw: 8 } }, size: Size { raw: 8 }, align: Align { pow2: 2 }, mutability: Not, extra: () }, offset: Size { raw: 0 } } : (u32, u32)`
struct ConstTuple<const M:(u32,u32)> {}

//struct ConstGenericTuple<T:PartialEq+Eq,const M:(T,T)> {}

#[derive(PartialEq,Eq)] struct TwoFieldStruct {pub x:u32, pub y:u32}
struct ConstTwoFieldStruct<const M:TwoFieldStruct> {}

#[derive(PartialEq, Eq)] struct TupleStruct(u32,u32);
struct ConstTupleStruct<const M:TupleStruct> {}
#[derive(PartialEq, Eq)] struct GenericTupleStruct<T>(T,T);
struct ConstGenericTupleStruct<const M:GenericTupleStruct<u32>> {}
#[derive(PartialEq, Eq)] struct GenericTwoFieldStruct<T>{x:T, y:T}
struct ConstGenericTwoFieldStruct<const M:GenericTwoFieldStruct<u32>> {}

fn main() {
    let _ = ConstGenericOneFieldStruct::<{GenericOneFieldStruct{x:0}}>{};
    let _ = ConstTwoParameters::<0,0>{};
    let _ = ConstTuple::<{(0,0)}>{};
    //let _ = ConstGenericTuple::<{(0,0)}>{};
    let _ = ConstTwoFieldStruct::<{TwoFieldStruct{x:0,y:0}}>{};
    let _ = ConstTupleStruct::<{TupleStruct(0,0)}>{};
    let _ = ConstGenericTupleStruct::<{GenericTupleStruct(0,0)}>{};
    let _ = ConstGenericTwoFieldStruct::<{GenericTwoFieldStruct{x:0,y:0}}>{};
}*/

/*#![allow(incomplete_features)]#![feature(const_generics)]
struct ConstTwoParameters<const X:u32,const Y:u32> {}

// Works

struct AnotherConstTwoParameters<const X:u32,const Y:u32> {}
impl<const X:u32, const Y:u32> AnotherConstTwoParameters<X,Y> {
    fn function(_: ConstTwoParameters<X,Y>) {}
    fn method(self, _: ConstTwoParameters<X,Y>) {}
}

struct ConstTuple<const M:(u32,u32)> {}
/*impl<const M:(u32,u32)> ConstTuple<M> {
    //fn function(_: ConstTwoParameters<X,Y>) {}
    fn method(self, _: ConstTwoParameters<{M.0},{M.1}>) {}
}*/

/*impl<const X:u32, const Y:u32> ConstTuple<{(X,Y)}> { // the const parameter `X` is not constrained by the impl trait, self type, or predicates
    fn method(self, _: ConstTwoParameters<X,Y>) {}
}*/

fn main() {
    //function(ConstTwoParameters::<0,0>{});
    //ConstTwoParameters::<0,0>::function(ConstTwoParameters::<0,0>{});
    //ConstTwoParameters::<0,0>{}.method(ConstTwoParameters::<0,0>{});
    //AnotherConstTwoParameters::<0,0>::function(ConstTwoParameters::<0,0>{});
    //AnotherConstTwoParameters{}.method(ConstTwoParameters::<0,0>{});
    //ConstTuple{}.method(ConstTwoParameters::<0,0>{}); // expected `{M.0}`, found `0u32`
    //ConstTuple{}.method(ConstTwoParameters::<0,0>{}); // expected `{M.0}`, found `0u32`
}*/

#![allow(incomplete_features)]
#![feature(const_generics)]
struct ConstGenericPrimitive<const T:u32>();
struct ConstGenericTuple1<const T:(u32,)>();
struct ConstGenericTuple2<const T:(u32,u32)>();
#[derive(PartialEq,Eq)] struct Struct();
struct ConstGenericStruct<const T:Struct>();
fn main() {
    let _ = ConstGenericPrimitive::<{0}>();
    let _ = ConstGenericTuple1::<{(0,)}>();
    let _ = ConstGenericTuple2::<{(0,0)}>();
    let _ = ConstGenericStruct::<{Struct()}>();
}

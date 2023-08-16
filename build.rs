fn main() {
    println!("cargo:rustc-cdylib-link-arg=build/dinput8.def");
    let res = winresource::WindowsResource::new();
    res.compile().unwrap();
}

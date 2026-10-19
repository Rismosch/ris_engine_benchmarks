use std::path::PathBuf;

use criterion::Criterion;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;

use ris_asset::RisGodAsset;

use ris_data::info::app_info::AppInfo;
use ris_data::info::args_info::ArgsInfo;
use ris_data::info::build_info::BuildInfo;
use ris_data::info::cpu_info::CpuInfo;
use ris_data::info::file_info::FileInfo;
use ris_data::info::sdl_info::SdlInfo;
use ris_core::god_object::GodObject;

fn asset(c: &mut Criterion) {
    let mut group = c.benchmark_group("asset_new");

    // construct engine
    let root_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let compiled_assets_path = root_dir.join("compiled_assets");


    let all_args = std::env::args().collect::<Vec<_>>();
    let executable = &all_args[0];
    let args = [
        executable,
        ris_data::info::args_info::ASSETS_ARG,
        &compiled_assets_path.display().to_string(),
    ];
    eprintln!("args: {:?}", args);

    let package_info = ris_data::package_info!();

    let args_info = ArgsInfo::parse(args).unwrap();
    let build_info = BuildInfo::new();
    let cpu_info = CpuInfo::new().unwrap();
    let file_info = FileInfo::new(&package_info).unwrap();
    let sdl_info = SdlInfo::new();

    let app_info = AppInfo::new(
        args_info,
        build_info,
        cpu_info,
        file_info,
        package_info,
        sdl_info,
    );

    let god_object = GodObject::new(app_info).unwrap();
    let asset_loader = god_object.asset_loader.to_weak();
    let god_asset_id = asset_loader.god_asset_id();

    group.bench_function("load god asset", |b| {
        b.iter(|| {
            //for _ in 0..1000 {
                let id = god_asset_id.clone();
                let future = unsafe {asset_loader.load_async::<RisGodAsset>(id).unwrap()};
                let god_asset = future.wait();

                let futures = unsafe {[
                    asset_loader.load_bin_async(god_asset.default_vert_spv).unwrap(),
                    asset_loader.load_bin_async(god_asset.default_frag_spv).unwrap(),
                ]};

                for future in futures {
                    let asset = future.wait();
                    black_box(asset);
                }
            //}
        });
    });
}

criterion_group!(benches, asset);
criterion_main!(benches);

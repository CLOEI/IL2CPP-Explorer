use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use il2cpp_core::analysis::Il2CppProject;
use il2cpp_core::binary::{BinaryImage, ElfImage};
use il2cpp_core::metadata::{Metadata, MetadataHeader};
use il2cpp_core::model::Method;
use il2cpp_core::registration::{MethodAddress, RegistrationInfo};

const TARGET_BINARY: &str = "./libil2cpp.so";
const TARGET_METADATA: &str = "./global-metadata.dat";
const DEFAULT_TYPE_LIMIT: usize = 50;

pub(crate) fn target(verbose: bool) -> Result<()> {
    let (binary, metadata) = load_target()?;
    let registration = RegistrationInfo::discover(&binary, &metadata)
        .context("failed to discover native registrations")?;
    let mapped_methods = count_mapped_methods(&binary, &metadata, &registration)?;
    println!("IL2CPP Explorer Target\n");
    print_binary_summary(Path::new(TARGET_BINARY), &binary);
    println!();
    print_metadata_summary(Path::new(TARGET_METADATA), &metadata);
    println!("\nParsing");
    println!("  Strings:      OK");
    println!("  Images:       OK ({})", metadata.images.len());
    println!("  Assemblies:   OK ({})", metadata.assemblies.len());
    println!("  Types:        OK ({})", metadata.types.len());
    println!("  Fields:       OK ({})", metadata.fields.len());
    println!("  Methods:      OK ({})", metadata.methods.len());
    println!("\nExample Images");
    for image in metadata.images.iter().take(3) {
        println!("  {}", image.name);
    }
    println!("\nStatus");
    println!("  Metadata parsing:       OK");
    println!("  Binary parsing:         OK");
    println!("  Registration discovery: OK");
    println!(
        "  Native address mapping: OK ({mapped_methods}/{} methods)",
        metadata.methods.len()
    );
    println!();
    print_registration_summary(&binary, &registration);
    if verbose {
        println!();
        print_modules(&registration);
        println!();
        print_metadata_tables(metadata.header());
    }
    Ok(())
}

pub(crate) fn inspect_target() -> Result<()> {
    let (binary, metadata) = load_target()?;
    let registration = RegistrationInfo::discover(&binary, &metadata)
        .context("failed to discover native registrations")?;
    println!("IL2CPP Explorer Target Inspection\n");
    print_binary_summary(Path::new(TARGET_BINARY), &binary);
    println!();
    print_metadata_summary(Path::new(TARGET_METADATA), &metadata);
    println!();
    print_metadata_tables(metadata.header());
    print_examples(&metadata);
    println!();
    print_registration_summary(&binary, &registration);
    println!();
    print_modules(&registration);
    Ok(())
}

pub(crate) fn binary(path: &Path) -> Result<()> {
    let binary = ElfImage::open(path)
        .with_context(|| format!("failed to inspect binary '{}'", path.display()))?;
    println!("IL2CPP Explorer Binary\n");
    print_binary_summary(path, &binary);
    println!("  Type:         {}", binary.kind());
    println!("  Image base:   {:#018x}", binary.image_base());
    println!("  Entry point:  {:#018x}", binary.entry_point());
    println!(
        "  Sections:     {} ({} inspectable)",
        binary.section_count(),
        binary.sections().len()
    );
    println!("  Segments:     {}", binary.segments().len());

    println!("\nSections");
    for section in binary.sections() {
        println!("  {}", section.name);
        match section.file_offset {
            Some(offset) => println!("    File: {offset:#018x}"),
            None => println!("    File: not stored"),
        }
        println!("    VA:   {:#018x}", section.virtual_address);
        println!("    Size: {:#018x}", section.size);
        println!("    Perm: {}", section.permissions);
    }

    println!("\nProgram Segments");
    for (index, segment) in binary.segments().iter().enumerate() {
        println!("  [{index}] {}", segment.kind);
        println!("    File:     {:#018x}", segment.file_offset);
        println!("    FileSize: {:#018x}", segment.file_size);
        println!("    VA:       {:#018x}", segment.virtual_address);
        println!("    MemSize:  {:#018x}", segment.virtual_size);
        println!("    Align:    {:#x}", segment.alignment);
        println!("    Perm:     {}", segment.permissions);
    }
    Ok(())
}

pub(crate) fn metadata(path: &Path, verbose: bool) -> Result<()> {
    let metadata = load_metadata(path)?;
    println!("IL2CPP Explorer Metadata\n");
    print_metadata_summary(path, &metadata);
    println!("\nContent");
    println!("  Images:      {}", metadata.images.len());
    println!("  Assemblies:  {}", metadata.assemblies.len());
    println!("  Types:       {}", metadata.types.len());
    println!("  Fields:      {}", metadata.fields.len());
    println!("  Methods:     {}", metadata.methods.len());
    println!("  Parameters:  {}", metadata.parameters.len());
    if verbose {
        println!();
        print_metadata_tables(metadata.header());
    }
    Ok(())
}

pub(crate) fn metadata_string(path: &Path, index: u32) -> Result<()> {
    let metadata = load_metadata(path)?;
    println!("{}", metadata.string(index)?);
    Ok(())
}

pub(crate) fn images(path: &Path) -> Result<()> {
    for image in load_metadata(path)?.images {
        println!("{}", image.name);
    }
    Ok(())
}

pub(crate) fn assemblies(path: &Path) -> Result<()> {
    for assembly in load_metadata(path)?.assemblies {
        println!("{}", assembly.name);
    }
    Ok(())
}

pub(crate) fn types(path: &Path, query: Option<&str>, all: bool) -> Result<()> {
    let metadata = load_metadata(path)?;
    let query = query.map(str::to_lowercase);
    let matches: Vec<_> = metadata
        .types
        .iter()
        .filter(|ty| {
            query
                .as_ref()
                .is_none_or(|query| full_type_name(ty).to_lowercase().contains(query.as_str()))
        })
        .collect();
    let shown = if all {
        matches.len()
    } else {
        matches.len().min(DEFAULT_TYPE_LIMIT)
    };
    for ty in &matches[..shown] {
        println!("{}", full_type_name(ty));
    }
    if shown < matches.len() {
        println!(
            "\nShowing {shown} of {} matches. Pass --all for complete output.",
            matches.len()
        );
    }
    Ok(())
}

pub(crate) fn type_info(path: &Path, name: &str) -> Result<()> {
    let metadata = load_metadata(path)?;
    let matches: Vec<_> = metadata
        .types
        .iter()
        .filter(|ty| ty.name == name || full_type_name(ty) == name)
        .collect();
    if matches.is_empty() {
        anyhow::bail!("no type matches '{name}'");
    }
    if matches.len() > 1 {
        println!("Multiple types match \"{name}\":\n");
        for (index, ty) in matches.iter().enumerate() {
            println!("{}. {}", index + 1, full_type_name(ty));
        }
        return Ok(());
    }

    let ty = matches[0];
    let image = &metadata.images[ty.image.0];
    let assembly = &metadata.assemblies[image.assembly.0];
    println!("{}\n", full_type_name(ty));
    println!("Assembly\n  {}\n", assembly.name);
    println!("Namespace\n  {}\n", ty.namespace);
    println!("Fields");
    for field in &ty.fields {
        println!("  {}", metadata.fields[field.0].name);
    }
    println!("\nMethods");
    for method in &ty.methods {
        let method = &metadata.methods[method.0];
        if method.parameters.is_empty() {
            println!("  {}()", method.name);
        } else {
            println!("  {}(...)", method.name);
        }
    }
    Ok(())
}

pub(crate) fn info(binary: &Path, metadata: &Path) -> Result<()> {
    let project = Il2CppProject::load(binary, metadata).with_context(|| {
        format!(
            "failed to load binary '{}' and metadata '{}'",
            binary.display(),
            metadata.display()
        )
    })?;
    println!("IL2CPP Explorer\n");
    println!("Binary");
    println!("  Format: {}", project.binary_format());
    println!("  Architecture: {}", project.architecture());
    println!("\nMetadata");
    println!("  Sanity:  {:#010X}", project.metadata().header().sanity);
    println!("  Version: {}", project.metadata_version());
    Ok(())
}

pub(crate) fn registrations(binary_path: &Path, metadata_path: &Path, verbose: bool) -> Result<()> {
    let (binary, metadata, registration) = load_native(binary_path, metadata_path)?;
    let mapped_methods = count_mapped_methods(&binary, &metadata, &registration)?;

    println!("IL2CPP Explorer Registrations\n");
    print_registration_summary(&binary, &registration);
    println!(
        "  Mapped methods:        {mapped_methods}/{}",
        metadata.methods.len()
    );
    if verbose {
        println!();
        print_modules(&registration);
    }
    Ok(())
}

pub(crate) fn method(binary_path: &Path, metadata_path: &Path, query: &str) -> Result<()> {
    let (binary, metadata, registration) = load_native(binary_path, metadata_path)?;
    let matches = find_methods(&metadata, query);
    if matches.is_empty() {
        anyhow::bail!("no method matches '{query}'");
    }

    println!("IL2CPP Explorer Methods\n");
    for method in matches.iter().take(DEFAULT_TYPE_LIMIT) {
        print_method(&binary, &metadata, &registration, method)?;
    }
    if matches.len() > DEFAULT_TYPE_LIMIT {
        println!(
            "Showing {} of {} matches. Use a metadata method index for one exact result.",
            DEFAULT_TYPE_LIMIT,
            matches.len()
        );
    }
    Ok(())
}

pub(crate) fn address(binary_path: &Path, metadata_path: &Path, value: &str) -> Result<()> {
    let virtual_address = parse_address(value)?;
    let (binary, metadata, registration) = load_native(binary_path, metadata_path)?;
    let relative_address = virtual_address
        .checked_sub(binary.image_base())
        .context("address is below the binary image base")?;
    let segment = binary
        .segments()
        .iter()
        .find(|segment| {
            segment.kind == "LOAD"
                && virtual_address
                    .checked_sub(segment.virtual_address)
                    .is_some_and(|relative| relative < segment.virtual_size)
        })
        .context("address is not mapped by a LOAD segment")?;
    let file_offset = binary.virtual_to_offset(virtual_address);

    println!("IL2CPP Explorer Address\n");
    println!("Address");
    println!("  VA:          {virtual_address:#018x}");
    println!("  RVA:         {relative_address:#018x}");
    match file_offset {
        Some(offset) => println!("  File offset: {offset:#018x}"),
        None => println!("  File offset: not stored"),
    }
    println!("  Permissions: {}", segment.permissions);

    let mut matches = Vec::new();
    for method in &metadata.methods {
        if registration
            .resolve_method(&binary, &metadata, method.id)?
            .is_some_and(|address| address.virtual_address == virtual_address)
        {
            matches.push(method);
        }
    }
    println!("\nMethods");
    if matches.is_empty() {
        println!("  No method starts at this address.");
    } else {
        for method in matches {
            println!(
                "  [{}] {}",
                method.id.0,
                full_method_name(&metadata, method)
            );
        }
    }
    Ok(())
}

fn load_target() -> Result<(ElfImage, Metadata)> {
    let binary = ElfImage::open(TARGET_BINARY).context("failed to load ./libil2cpp.so")?;
    let metadata =
        Metadata::open(TARGET_METADATA).context("failed to load ./global-metadata.dat")?;
    Ok((binary, metadata))
}

fn load_native(
    binary_path: &Path,
    metadata_path: &Path,
) -> Result<(ElfImage, Metadata, RegistrationInfo)> {
    let binary = ElfImage::open(binary_path)
        .with_context(|| format!("failed to load binary '{}'", binary_path.display()))?;
    let metadata = load_metadata(metadata_path)?;
    let registration = RegistrationInfo::discover(&binary, &metadata)
        .context("failed to discover native registrations")?;
    Ok((binary, metadata, registration))
}

fn load_metadata(path: &Path) -> Result<Metadata> {
    Metadata::open(path).with_context(|| format!("failed to parse metadata '{}'", path.display()))
}

fn print_binary_summary(path: &Path, binary: &ElfImage) {
    println!("Binary");
    println!("  Path:         {}", display_path(path));
    println!("  Size:         {} bytes", binary.file_size());
    println!("  Format:       {}", binary.format());
    println!("  Architecture: {}", binary.architecture());
    println!("  Endianness:   {}", binary.endianness());
    println!(
        "  Stripped:     {}",
        if binary.is_stripped() { "Yes" } else { "No" }
    );
}

fn print_metadata_summary(path: &Path, metadata: &Metadata) {
    println!("Metadata");
    println!("  Path:         {}", display_path(path));
    println!("  Size:         {} bytes", metadata.file_size());
    println!("  Sanity:       {:#010X}", metadata.header().sanity);
    println!("  Version:      {}", metadata.version);
    println!("  Status:       Valid");
}

fn print_metadata_tables(header: &MetadataHeader) {
    println!("Metadata Tables");
    for (name, table) in header.tables() {
        println!("\n{name}");
        println!("  Offset:      {:#010x}", table.offset);
        println!("  Byte Count:  {:#010x}", table.byte_count);
    }
}

fn print_examples(metadata: &Metadata) {
    let sizes = metadata.record_sizes();
    println!("\nRecord Sizes (v31)");
    println!("  Images:      {}", sizes.image);
    println!("  Assemblies:  {}", sizes.assembly);
    println!("  Types:       {}", sizes.type_definition);
    println!("  Fields:      {}", sizes.field);
    println!("  Methods:     {}", sizes.method);
    println!("  Parameters:  {}", sizes.parameter);
    println!("\nFirst 5 Images");
    for (index, image) in metadata.images.iter().take(5).enumerate() {
        println!("  [{index}] {}", image.name);
    }
    println!("\nFirst 5 Assemblies");
    for (index, assembly) in metadata.assemblies.iter().take(5).enumerate() {
        println!("  [{index}] {}", assembly.name);
    }
    println!("\nFirst 5 Types");
    for (index, ty) in metadata.types.iter().take(5).enumerate() {
        println!("  [{index}] {}", full_type_name(ty));
    }
    println!("\nFirst 5 Methods");
    for (index, method) in metadata.methods.iter().take(5).enumerate() {
        let ty = &metadata.types[method.declaring_type.0];
        println!("  [{index}] {}::{}", full_type_name(ty), method.name);
    }
}

fn print_registration_summary(binary: &ElfImage, registration: &RegistrationInfo) {
    println!("Registrations");
    print_optional_native_address(
        "Code registration",
        registration.registration.code_registration,
        binary,
    );
    print_optional_native_address(
        "Metadata registration",
        registration.registration.metadata_registration,
        binary,
    );
    print_native_address("CodeGenModule array", registration.codegen_modules, binary);
    println!("  Modules:               {}", registration.modules.len());
    println!(
        "  Method pointer slots:  {}",
        registration
            .modules
            .iter()
            .map(|module| u64::from(module.method_pointer_count))
            .sum::<u64>()
    );
}

fn print_modules(registration: &RegistrationInfo) {
    println!("CodeGenModules");
    for (index, module) in registration.modules.iter().enumerate() {
        let pointers = module
            .method_pointers
            .map_or_else(|| "none".to_owned(), |address| format!("{address:#x}"));
        println!(
            "  [{index:>2}] {:<48} module={:#x} methods={} pointers={pointers}",
            module.name, module.address, module.method_pointer_count
        );
    }
}

fn print_optional_native_address(label: &str, address: Option<u64>, binary: &ElfImage) {
    match address {
        Some(address) => print_native_address(label, address, binary),
        None => println!("  {label:<22} not found"),
    }
}

fn print_native_address(label: &str, address: u64, binary: &ElfImage) {
    let file = binary
        .virtual_to_offset(address)
        .map_or_else(|| "not stored".to_owned(), |offset| format!("{offset:#x}"));
    println!("  {label:<22} VA={address:#x} file={file}");
}

fn count_mapped_methods(
    binary: &ElfImage,
    metadata: &Metadata,
    registration: &RegistrationInfo,
) -> Result<usize> {
    let mut mapped = 0;
    for method in &metadata.methods {
        if registration
            .resolve_method(binary, metadata, method.id)?
            .is_some()
        {
            mapped += 1;
        }
    }
    Ok(mapped)
}

fn find_methods<'a>(metadata: &'a Metadata, query: &str) -> Vec<&'a Method> {
    if query.bytes().all(|byte| byte.is_ascii_digit()) {
        return query
            .parse::<usize>()
            .ok()
            .and_then(|index| metadata.methods.get(index))
            .into_iter()
            .collect();
    }

    let query = query.to_lowercase();
    metadata
        .methods
        .iter()
        .filter(|method| {
            full_method_name(metadata, method)
                .to_lowercase()
                .contains(&query)
        })
        .collect()
}

fn print_method(
    binary: &ElfImage,
    metadata: &Metadata,
    registration: &RegistrationInfo,
    method: &Method,
) -> Result<()> {
    let declaring_type = &metadata.types[method.declaring_type.0];
    let image = &metadata.images[declaring_type.image.0];
    println!("[{}] {}", method.id.0, full_method_name(metadata, method));
    println!("  Image:        {}", image.name);
    println!("  Token:        {:#010x}", method.token);
    match registration.resolve_method(binary, metadata, method.id)? {
        Some(address) => print_method_address(&address),
        None => println!("  Native:       not generated"),
    }
    println!();
    Ok(())
}

fn print_method_address(address: &MethodAddress) {
    println!("  Pointer index: {}", address.pointer_index);
    println!("  VA:            {:#018x}", address.virtual_address);
    println!("  RVA:           {:#018x}", address.relative_address);
    println!("  File offset:   {:#018x}", address.file_offset);
}

fn full_method_name(metadata: &Metadata, method: &Method) -> String {
    let declaring_type = &metadata.types[method.declaring_type.0];
    format!("{}::{}", full_type_name(declaring_type), method.name)
}

fn parse_address(value: &str) -> Result<u64> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).with_context(|| format!("invalid address '{value}'"))
    } else {
        value
            .parse::<u64>()
            .with_context(|| format!("invalid address '{value}'"))
    }
}

fn full_type_name(ty: &il2cpp_core::model::TypeDefinition) -> String {
    if ty.namespace.is_empty() {
        ty.name.clone()
    } else {
        format!("{}.{}", ty.namespace, ty.name)
    }
}

fn display_path(path: &Path) -> String {
    let value = path.display().to_string();
    if value.starts_with('.') || path.is_absolute() {
        value
    } else {
        PathBuf::from(".").join(path).display().to_string()
    }
}

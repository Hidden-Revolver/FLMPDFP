use arboard::Clipboard;
use clap::{Arg, ArgAction, Command};
use csv::Writer;
use indicatif::{ProgressBar, ProgressStyle};
use pdf_extract::extract_text;
use regex::Regex;
use std::path::Path;
use std::{fs, io};
struct PdfInfo {
    fecha: String,
    total: String,
    cliente: String,
}

fn extract_info_pdf(pdf_path: &Path, pb: &ProgressBar) -> PdfInfo {
    let text = extract_text(pdf_path).unwrap();

    let name_pattern = Regex::new(r"CLIENTE:\s*[a-zA-ZáéíóúñÑÁÉÍÓÚ., ]+").unwrap();
    let date_pattern = Regex::new(r"\d{2}/\d{2}/\d{4}").unwrap();
    let total_pattern = Regex::new(r"TOTAL A FACTURAR\s*[\d,.]+\d{2}€").unwrap();

    let fecha = date_pattern.find(&text).map_or("No encontrado".to_string(),|m| m.as_str().to_string().trim().to_string());
    if fecha == "No encontrado" {
        let st=format!("ALERTA: Fecha no encontrada en el pdf: {}", pdf_path.display());
        pb.println(st);
    }
    let total = total_pattern.find(&text).map_or("No encontrado".to_string(),|m| m.as_str()[17..].to_string().trim().to_string());
    if total == "No encontrado" {
        let st=format!("ALERTA: Importe no encontrado en el pdf: {}", pdf_path.display());
        pb.println(st);
    }
    let cliente = name_pattern.find(&text).map_or("No encontrado".to_string(),|m| m.as_str()[9..].to_string().trim().to_string());
    if cliente == "No encontrado" {
        let st=format!("ALERTA: Cliente no encontrada en el pdf: {}", pdf_path.display());
        pb.println(st);
    }
    PdfInfo {
        fecha,
        total,
        cliente,
    }
}
fn ordenar(resultados: &mut [PdfInfo], col: &str) {
    match col {
        "Fecha" => {
            resultados.sort_by(|a, b| {
                let partesa: Vec<&str> = a.fecha.split("/").collect();
                let partesb: Vec<&str> = b.fecha.split("/").collect();
                (partesa[2].parse::<i32>().unwrap() * 10000 // esto es optimizable
                    + partesa[1].parse::<i32>().unwrap() * 100
                    + partesa[0].parse::<i32>().unwrap())
                .cmp(
                    &(partesb[2].parse::<i32>().unwrap() * 10000
                        + partesb[1].parse::<i32>().unwrap() * 100
                        + partesb[0].parse::<i32>().unwrap()),
                )
            });
        }
        "Importe" => {
            resultados.sort_by(|a, b| {
                let mut numa: f32 = f32::NAN;
                let mut numb: f32 = f32::NAN;
                if !a.total.contains("No") {
                    numa = a.total.replace(".", "").parse::<f32>().unwrap();
                }
                if !b.total.contains("No") {
                    numb = b.total.replace(".", "").parse::<f32>().unwrap();
                }
                numa.partial_cmp(&numb).unwrap()
            });
        }
        "Cliente" => {
            resultados.sort_by(|a, b| a.cliente.cmp(&b.cliente));
        }
        &_ => {
            eprintln!("ERROR: col {} not found", col);
        }
    }
}
fn crear_tabla(resultados: &mut Vec<PdfInfo>, col: &str) {
    let mut wtr = Writer::from_path("resultados.csv").unwrap();
    wtr.write_record(["Fecha", "Importe", "Cliente"]).unwrap();
    ordenar(resultados, col);

    for info in resultados {
        wtr.write_record([&info.fecha, &info.total, &info.cliente])
            .unwrap();
    }

    wtr.flush().unwrap();
    println!("Tabla guardada en resultados.csv");
}

fn copiar_al_portapapeles(datos: &str) -> Result<(), Box<dyn std::error::Error>> {
    // no funciona al parecer en linux
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(datos)?;
    println!("Tabla copiada al portapapeles");
    Ok(())
}

fn formatear_como_tabla(resultados: &mut Vec<PdfInfo>, col: &str) -> String {
    let mut tabla = String::from("Fecha\tImporte\tCliente\n");
    ordenar(resultados, col);
    for info in resultados {
        tabla.push_str(&format!(
            "{}\t{}\t{}\n",
            info.fecha, info.total, info.cliente
        ));
    }
    tabla
}
fn main() {
    println!("Procesamiento de PDFs\n");
    /*
    let args: Vec<String> = env::args().collect();
    let verbose = args.contains(&"-v".to_string()) || args.contains(&"--verbose".to_string());
    let ordenar_cliente = args.contains(&"-oc".to_string()) || args.contains(&"--ordenar_cliente".to_string());
    let ordenar_precio = args.contains(&"-op".to_string()) || args.contains(&"--ordenar_precio".to_string());
    let csv=args.contains(&"-c".to_string()) || args.contains(&"--csv".to_string());
    let help=args.contains(&"-h".to_string()) || args.contains(&"--help".to_string());
     */
    let matches = Command::new("Procesamiento de PDFs")
        .version("2.5.4")
        .author("Carlos Manzanedo Sola")
        .about("Procesa archivos PDF de importe y los copia directamente en tu portapapeles")
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("Muestra los resultados del análisis por pantalla"),
        )
        .arg(
            Arg::new("ordenar_cliente")
                .short('c')
                .long("ordenar_cliente")
                .action(ArgAction::SetTrue)
                .help("Ordena la tabla de salida por cliente"),
        )
        .arg(
            Arg::new("ordenar_precio")
                .short('p')
                .long("ordenar_precio")
                .action(ArgAction::SetTrue)
                .help("Ordena la tabla de salida por precio"),
        )
        .arg(
            Arg::new("csv")
                .short('o')
                .long("csv")
                .action(ArgAction::SetTrue)
                .help("Exporta el contenido a un csv"),
        )
        .get_matches();

    let res = if matches.get_flag("ordenar_precio") {
        "Importe"
    } else if matches.get_flag("ordenar_cliente") {
        "Cliente"
    } else {
        "Fecha"
    };

    let carpeta = Path::new("./archivos");
    let mut resultados = Vec::new();

    let pdfs: Vec<_> = fs::read_dir(carpeta)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pdf")
                && !path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("delete")
                && !path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("DELETE")
                && !path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("Delete")
                && path.file_name().unwrap() != "dele.pdf"
            {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    let pb = ProgressBar::new(pdfs.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}]|[{eta_precise}]: [{bar:40.cyan/blue}] {pos:>7}/{len:7} [{msg}] {per_sec}")
            .expect("REASON")
            .progress_chars("# "),
    );
    if res == "Importe" {
        pb.set_message("Ordenando por importe");
    } else if res == "Cliente" {
        pb.set_message("Ordenando por cliente")
    } else {
        pb.set_message("Ordenando por fecha")
    }
    for pdf_path in pdfs {
        let info = extract_info_pdf(&pdf_path,&pb);
        resultados.push(info);
        pb.inc(1);
    }
    pb.finish_with_message("Procesamiento completado");

    if matches.get_flag("verbose") {
        for info in &resultados {
            println!(
                "Importe: {}\nFecha: {}\nCliente: {}\n",
                info.total, info.fecha, info.cliente
            );
        }
    }
    if matches.get_flag("csv") {
        crear_tabla(&mut resultados, res);
    }
    let tabla_formateada = formatear_como_tabla(&mut resultados, res);
    copiar_al_portapapeles(&tabla_formateada).unwrap();
    if matches.get_flag("verbose") {
        println!("Datos copiados: 3 columnas, {} filas", resultados.len());
    }

    println!("Presiona Enter para salir...");
    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("Falló la lectura");
}

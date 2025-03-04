use crate::VecPdfInfo::{Em, Rec};
use arboard::Clipboard;
use clap::{Arg, ArgAction, ArgMatches, Command};
use csv::Writer;
use indicatif::{ProgressBar, ProgressStyle};
use pdf_extract::extract_text;
use regex::Regex;
use std::path::Path;
use std::{fs, io};

struct PdfInfoEm {
    fecha: String,
    total: String,
    cliente: String,
}
struct PdfInfoRec {
    concepto: String,
    fecha: String,
    nif: String,
    empresa: String,
    base: String,
    iva: String,
    total: f32,
}
enum VecPdfInfo {
    Rec(Vec<PdfInfoRec>),
    Em(Vec<PdfInfoEm>),
}

fn procesar_pdf_em(pdf_path: &Path, pb: &ProgressBar) -> PdfInfoEm {
    let text = extract_text(pdf_path).unwrap();

    let name_pattern = Regex::new(r"CLIENTE:\s*[a-zA-ZáéíóúñÑÁÉÍÓÚ., ]+").unwrap();
    let date_pattern = Regex::new(r"\d{2}/\d{2}/\d{4}").unwrap();
    let total_pattern = Regex::new(r"TOTAL A FACTURAR\s*[\d,.]+\d{2}€").unwrap();

    let fecha = date_pattern
        .find(&text)
        .map_or("No encontrado".to_string(), |m| {
            m.as_str().to_string().trim().to_string()
        });
    if fecha == "No encontrado" {
        let st = format!(
            "ALERTA: Fecha no encontrada en el pdf: {}",
            pdf_path.display()
        );
        pb.println(st);
    }
    let total = total_pattern
        .find(&text)
        .map_or("No encontrado".to_string(), |m| {
            m.as_str()[17..].to_string().trim().to_string()
        });
    if total == "No encontrado" {
        let st = format!(
            "ALERTA: Importe no encontrado en el pdf: {}",
            pdf_path.display()
        );
        pb.println(st);
    }
    let cliente = name_pattern
        .find(&text)
        .map_or("No encontrado".to_string(), |m| {
            m.as_str()[9..].to_string().trim().to_string()
        });
    if cliente == "No encontrado" {
        let st = format!(
            "ALERTA: Cliente no encontrada en el pdf: {}",
            pdf_path.display()
        );
        pb.println(st);
    }
    PdfInfoEm {
        fecha,
        total,
        cliente,
    }
}

fn ordenar(resultados: &mut [PdfInfoEm], col: &str) {
    match col {
        "Fecha" => {
            fn getfechacmp(fech: &str) -> i32 {
                let partes: Vec<&str> = fech.split("/").collect();
                partes[2].parse::<i32>().unwrap() * 10000
                    + partes[1].parse::<i32>().unwrap() * 100
                    + partes[0].parse::<i32>().unwrap()
            }
            resultados
                .sort_by(|a, b| getfechacmp(a.fecha.as_str()).cmp(&getfechacmp(b.fecha.as_str())));
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

fn crear_tabla(resultados: &mut Vec<PdfInfoEm>, col: &str) {
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

fn formatear_como_tabla(resultados: &mut Vec<PdfInfoEm>, col: &str) -> String {
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

fn get_arg_matches() -> ArgMatches {
    Command::new("Procesamiento de PDFs")
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
        .arg(
            Arg::new("procesar_recibidos")
                .short('r')
                .long("procesar_recibidos")
                .action(ArgAction::SetTrue)
                .help("Procesa los PDFs recibidos"),
        )
        .get_matches()
}

fn main() {
    println!("Procesamiento de PDFs\n");

    let matches = get_arg_matches();

    let res = if matches.get_flag("ordenar_precio") {
        "Importe"
    } else if matches.get_flag("ordenar_cliente") {
        "Cliente"
    } else {
        "Fecha"
    };

    let carpeta = if matches.get_flag("procesar_recibidos") {
        Path::new("./Recibidos")
    } else {
        Path::new("./Emitidos")
    };
    let resultados = if matches.get_flag("procesar_recibidos") {
        Rec(Vec::new())
    } else {
        Em(Vec::new())
    };

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
            .expect("ERROR")
            .progress_chars("# "),
    );
    if res == "Importe" {
        pb.set_message("Ordenando por importe");
    } else if res == "Cliente" {
        pb.set_message("Ordenando por cliente")
    } else {
        pb.set_message("Ordenando por fecha")
    }
    match resultados {
        Rec(ref mut vec) => {
            todo!()
        }
        Em(ref mut vec) => {
            for pdf_path in pdfs {
                let info = procesar_pdf_em(&pdf_path, &pb);
                vec.push(info);
                pb.inc(1);
            }
        }
    }
    pb.finish_with_message("Procesamiento completado");

    if matches.get_flag("verbose") {
        match resultados {
            Rec(ref vec) => {
                todo!()
            }
            Em(ref vec) => {
                for info in vec {
                    println!(
                        "Importe: {}\nFecha: {}\nCliente: {}\n",
                        info.total, info.fecha, info.cliente
                    );
                }
            }
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

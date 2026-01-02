use quote::{format_ident, quote};
use regex::Regex;
use std::{env, fs, path::Path};

//
fn generate_code_for_event_queue(req: &GenerateRequest) -> String {
    let contents = std::fs::read_to_string(req.source).unwrap();

    // loop through the source file and look for all struct definitions.
    let all_types: Vec<_> = req
        .struct_search_regex
        .captures_iter(&contents)
        .map(|x| format_ident!("{}", &x[1]))
        .collect();

    //let all_types_lowercase: Vec<_> = all_types
    //.iter()
    //.map(|x| format_ident!("writer_{}", x.to_string().to_lowercase()))
    //.collect();

    let incoming_typename = format_ident!("EventTo{}", req.incoming_type_name);
    let outgoing_typename = format_ident!("EventTo{}", req.outgoing_type_name);

    let code = quote!(
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[non_exhaustive]
        pub enum #incoming_typename {
            #( #all_types ( #all_types ) ),*
        }

        pub fn drain_incoming_events (
            world: &mut World,
        ) {
            let sr = world.resource::<NetworkingResources<#incoming_typename, crate::netlib:: #outgoing_typename>>().clone();
            let mut new_events = sr.event_list_incoming_udp.write().unwrap();
            let new_events = std::mem::replace(new_events.as_mut(), vec![]);
            for (endpoint, event) in new_events {
                trace!(?event, "Received event from endpoint {:?}", endpoint);
                match event {
                    #(
                        #incoming_typename :: #all_types (data) => {
                            world.write_message(EventFromEndpoint::new(endpoint, data));
                        }
                    ),*
                }
            }
        }

        pub struct NetworkEventPlugin;

        impl Plugin for NetworkEventPlugin {
            fn build(&self, app: &mut App) {
                #(
                    app.add_message::< EventFromEndpoint< #all_types > >();
                )*
            }
        }
    );

    let code = syn::parse_file(&code.to_string()).unwrap();

    prettyplease::unparse(&code)
    //code.to_string()
}

fn generate_systems_for_event_queue(req: GenerateRequest) {
    let code_str = generate_code_for_event_queue(&req);

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join(req.output_filename);
    fs::write(dest_path, code_str).unwrap();
}
struct GenerateRequest<'a> {
    output_filename: &'a str,
    incoming_type_name: &'a str,
    outgoing_type_name: &'a str,
    source: &'a str,
    struct_search_regex: &'a Regex,
}

fn main() {
    let r = Regex::new(r#"(?:struct|enum) (\w+?) \{"#).unwrap();
    generate_systems_for_event_queue(GenerateRequest {
        source: "src/event/client.rs",
        output_filename: "./client_event.rs",
        incoming_type_name: "Client",
        outgoing_type_name: "Server",
        struct_search_regex: &r,
    });

    generate_systems_for_event_queue(GenerateRequest {
        source: "src/event/server.rs",
        output_filename: "./server_event.rs",
        incoming_type_name: "Server",
        outgoing_type_name: "Client",
        struct_search_regex: &r,
    });

    //generate_systems_for_shared_components(GenerateRequest {
    //source: "src/event/shared_components.rs",
    //output_filename: "./shared_components.rs",
    //output_type_name: "SharedComponents",
    //struct_search_regex: &r,
    //});

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/event/server.rs");
    println!("cargo:rerun-if-changed=src/event/client.rs");
}

// use consul::Client;
// use consul::catalog::CatalogService;
//
// fn get_service_instances(service_name: &str) -> Vec<String> {
//     let client = Client::new("http://localhost:8500");
//     let services = client.catalog().service(service_name, None).unwrap();
//     services
//         .into_iter()
//         .map(|s: CatalogService| format!("{}:{}", s.address, s.service_port))
//         .collect()
// }
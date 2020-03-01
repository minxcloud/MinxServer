use actix_web::{get, post, web, App, HttpServer, Responder}; //HttpRequest, HttpResponse
use serde::{Serialize, Deserialize};
use std::io;
use std::sync::Arc;

#[derive(Clone)]
struct MyAppState {
	nodes: Arc<Vec<NodeInfo>>
}

#[derive(Serialize, Deserialize, Debug)]
struct NodeInfo {
	node: String,
	node_addr: String,
	doc_url: String,
	services: Vec<String>,
	dependence: Vec<String>
}

#[get ("/")]
async fn _index () -> &'static str {
	"hello world"
}

#[post ("/_minx_/register")]//info: web::Path<(u32, String)>    req: HttpRequest
async fn _register (_state: web::Data<MyAppState>, _node_info: web::Json<NodeInfo>) -> impl Responder {
	_state.nodes.
	String::from (&_node_info.doc_url[..])
}

#[actix_rt::main]
async fn main () -> io::Result<()> {
	let _state = MyAppState {
		nodes: Arc::from(Vec::new ())
	};
	HttpServer::new (
		move || App::new ().app_data (_state.clone ()).service (_index)
	).bind ("127.0.0.1:8088")?.run ().await
	//HttpServer::new(|| {
	//	App::new()
	//	.route("/", web::get().to(index))
	//	.route("/again", web::get().to(index2))
	//}).bind("127.0.0.1:8088")?.run().await
}

use yew::prelude::*;
use wasm_bindgen::prelude::*;
use web_sys::HtmlElement;

// Dummy connection type.
#[derive(Clone, PartialEq, Debug)]
struct Connection {
    id: String,
    address: String,
}

#[function_component(ConnectionList)]
fn connection_list() -> Html {
    let connections = use_state(|| vec![
        Connection { id: "1".into(), address: "10.0.0.2".into() },
        Connection { id: "2".into(), address: "10.0.0.3".into() },
    ]);
    let selected_conn = use_state(|| None::<Connection>);

    let on_select = {
        let selected_conn = selected_conn.clone();
        Callback::from(move |conn: Connection| selected_conn.set(Some(conn)))
    };

    html! {
        <div>
            <h2>{ "Active Connections" }</h2>
            <ul>
                { for connections.iter().map(|conn| {
                    let conn_clone = conn.clone();
                    let on_click = {
                        let on_select = on_select.clone();
                        Callback::from(move |_| on_select.emit(conn_clone.clone()))
                    };
                    html! {
                        <li {on_click}>{ format!("ID: {} – Addr: {}", conn.id, conn.address) }</li>
                    }
                }) }
            </ul>
            {
                if let Some(conn) = &*selected_conn {
                    html! { <Terminal connection={conn.clone()} /> }
                } else {
                    html! { <p>{ "Select a connection to open a terminal" }</p> }
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct TerminalProps {
    connection: Connection,
}

#[function_component(Terminal)]
fn terminal(props: &TerminalProps) -> Html {
    let terminal_div_ref = use_node_ref();
    let connection_id = props.connection.id.clone();

    {
        let terminal_div_ref = terminal_div_ref.clone();
        use_effect_with_deps(move |_| {
            if let Some(div) = terminal_div_ref.cast::<HtmlElement>() {
                // Connect via secure WebSocket (wss) on port 443.
                let ws_url = format!("wss://yourdomain.com/terminal/{}", connection_id);
                init_terminal(&div, &ws_url);
            }
            || ()
        }, ());
    }

    html! {
        <div>
            <h3>{ format!("Terminal for Connection {}", connection_id) }</h3>
            <div ref={terminal_div_ref} style="width: 100%; height: 400px; background: black;"></div>
        </div>
    }
}

#[wasm_bindgen(module = "/src/terminal.js")]
extern "C" {
    fn init_terminal(container: &HtmlElement, ws_url: &str);
}

#[function_component(App)]
fn app() -> Html {
    html! {
        <div>
            <h1>{ "Secure Chat Server Dashboard" }</h1>
            <ConnectionList />
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}

import { Terminal } from 'xterm';
import 'xterm/css/xterm.css';

export function init_terminal(container, ws_url) {
  const term = new Terminal({
    cursorBlink: true,
    scrollback: 1000,
    tabStopWidth: 8,
  });
  term.open(container);

  // Connect via secure WebSocket.
  const socket = new WebSocket(ws_url);
  socket.binaryType = 'arraybuffer';

  socket.addEventListener('open', () => {
    term.writeln('Connected to secure endpoint I/O stream');
  });

  socket.addEventListener('message', (event) => {
    if (typeof event.data === 'string') {
      term.write(event.data);
    } else {
      const text = new TextDecoder("utf-8").decode(event.data);
      term.write(text);
    }
  });

  socket.addEventListener('error', (event) => {
    term.writeln(`\r\nError: ${event.message || 'Unknown error'}`);
  });

  socket.addEventListener('close', () => {
    term.writeln('\r\nConnection closed');
  });

  term.onData((data) => {
    if (socket.readyState === WebSocket.OPEN) {
      socket.send(data);
    }
  });

  return { term, socket };
}

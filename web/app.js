const output = document.querySelector('#output');
document.querySelector('#serve').addEventListener('click', () => {
  output.textContent = 'Starting local server...';
  window.PocketTerm?.startServer();
});
document.querySelector('#stop').addEventListener('click', () => {
  window.PocketTerm?.stopServer();
  output.textContent = 'Server stopped.';
});

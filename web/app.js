// The page asks the API for every result. Nothing is computed here.

const termOne = document.getElementById("term_one");
const termTwo = document.getElementById("term_two");
const buttons = Array.from(document.querySelectorAll(".ops button"));
const result = document.getElementById("result");
const error = document.getElementById("error");
const request = document.getElementById("request");

let operation = "div";
let timer = null;
let inflight = null;

function select(next) {
  operation = next;
  for (const button of buttons) {
    button.setAttribute("aria-pressed", String(button.dataset.op === operation));
  }
  schedule(0);
}

// A short pause after the last keystroke, so a number typed digit by digit
// becomes one request rather than six.
function schedule(delay = 150) {
  clearTimeout(timer);
  timer = setTimeout(compute, delay);
}

async function compute() {
  const a = termOne.value.trim();
  const b = termTwo.value.trim();
  if (a === "" || b === "") {
    render({});
    return;
  }

  const url = `/api/${operation}?${new URLSearchParams({ term_one: a, term_two: b })}`;
  if (inflight) inflight.abort();
  const controller = new AbortController();
  inflight = controller;

  let response;
  let body;
  try {
    response = await fetch(url, { signal: controller.signal });
    body = await response.text();
  } catch (err) {
    if (err.name === "AbortError") return;
    render({ url, error: "the API did not answer" });
    return;
  }
  if (controller !== inflight) return;
  render({ url, status: response.status, body });
}

function render({ url, status, body, error: failure }) {
  result.textContent = "";
  result.removeAttribute("title");
  error.hidden = true;
  error.textContent = "";
  request.textContent = url ? `GET ${url}${status ? ` → ${status} ${body}` : ""}` : "";

  if (failure) {
    error.textContent = failure;
    error.hidden = false;
    return;
  }
  if (!body) return;

  // Results can go past Number.MAX_SAFE_INTEGER, and JSON.parse would round
  // them. Read the digits straight from the body instead.
  const digits = body.match(/"result"\s*:\s*(-?\d+)/);
  if (status === 200 && digits) {
    result.textContent = group(digits[1]);
    result.title = digits[1];
    return;
  }

  let message = `HTTP ${status}`;
  try {
    message = JSON.parse(body).error || message;
  } catch {
    // Not JSON: keep the status.
  }
  error.textContent = message;
  error.hidden = false;
}

// Thin spaces every three digits, for reading. The title keeps the plain number.
function group(number) {
  const sign = number.startsWith("-") ? "-" : "";
  const digits = number.slice(sign.length);
  return sign + digits.replace(/\B(?=(\d{3})+(?!\d))/g, "\u2009");
}

for (const button of buttons) {
  button.addEventListener("click", () => select(button.dataset.op));
}
for (const input of [termOne, termTwo]) {
  input.addEventListener("input", () => schedule());
}
document.getElementById("calc").addEventListener("submit", (event) => {
  event.preventDefault();
  schedule(0);
});

compute();

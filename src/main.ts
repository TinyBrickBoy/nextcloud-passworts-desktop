import { mount } from "svelte";
import App from "./App.svelte";
import "@fontsource-variable/inter";
import "./app.css";

export default mount(App, { target: document.getElementById("app")! });

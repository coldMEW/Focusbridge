/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: {
          primary: "#f4efe4",
          secondary: "#fffaf0",
          surface: "#ece3d1",
        },
        text: {
          primary: "#17221e",
          secondary: "#61706a",
          muted: "#9a8f7c",
        },
        border: {
          subtle: "rgba(40, 54, 48, 0.14)",
          hover: "rgba(40, 54, 48, 0.34)",
        },
        accent: {
          study: "#3f7f70",
          important: "#c97b38",
        },
      },
      fontFamily: {
        sans: ["Aptos", "Gill Sans", "Trebuchet MS", "sans-serif"],
      },
    },
  },
  plugins: [],
};

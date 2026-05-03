/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: {
          primary: "#1A1A1A",
          secondary: "#2D2D2D",
          surface: "#363636",
        },
        text: {
          primary: "#E0E0E0",
          secondary: "#888888",
          muted: "#555555",
        },
        border: {
          subtle: "#404040",
          hover: "#505050",
        },
        accent: {
          study: "#6B7B8D",
          important: "#A0A0A0",
        },
      },
      fontFamily: {
        sans: [
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "Roboto",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
};

declare module "*.pem?raw" { const text: string; export default text; }
declare namespace Cloudflare {
  interface Env extends import("../src/index").Env {}
  interface GlobalProps {
    mainModule: typeof import("../src/index");
  }
}

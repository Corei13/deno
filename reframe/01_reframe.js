import { op_reframe_analyze } from "ext:core/ops";

export default {
  async analyzeAsync(path, content, env = "server", minify = true) {
    const json = await op_reframe_analyze(path, content, env, minify);
    return JSON.parse(json);
  },
};

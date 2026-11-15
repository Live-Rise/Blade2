function resize(canvas, width, height) {
    if (canvas.width === width && canvas.height === height)
        return false;
    canvas.width = width;
    canvas.height = height;
    return true;
}
function canvasPainter(canvas) {
    const context = canvas.getContext('2d', { alpha: true });
    if (context === null)
        throw new Error('Canvas2D is unavailable.');
    let disposed = false;
    return {
        backend: 'canvas2d', canvas,
        async paint(frame) {
            if (disposed)
                throw new Error('Native painter disposed.');
            resize(canvas, frame.displayWidth, frame.displayHeight);
            context.clearRect(0, 0, canvas.width, canvas.height);
            context.drawImage(frame, 0, 0);
        },
        dispose() { disposed = true; },
    };
}
async function gpuPainter(canvas) {
    const gpu = navigator.gpu;
    if (gpu === undefined)
        throw new Error('WebGPU is unavailable.');
    const adapter = await gpu.requestAdapter();
    if (adapter === null)
        throw new Error('WebGPU adapter is unavailable.');
    const device = await adapter.requestDevice();
    try {
        const context = canvas.getContext('webgpu');
        if (context === null)
            throw new Error('WebGPU canvas is unavailable.');
        const format = gpu.getPreferredCanvasFormat();
        // Electron external textures sample premultiplied BGRA; the canvas consumes that representation directly.
        const module = device.createShaderModule({ code: `
      struct Vertex { @builtin(position) position: vec4f, @location(0) uv: vec2f }
      @vertex fn vertex(@builtin(vertex_index) i: u32) -> Vertex {
        let p = vec2f(f32((i << 1u) & 2u), f32(i & 2u));
        return Vertex(vec4f(p * 2.0 - 1.0, 0.0, 1.0), vec2f(p.x, 1.0 - p.y));
      }
      @group(0) @binding(0) var image: texture_external;
      @group(0) @binding(1) var imageSampler: sampler;
      @fragment fn fragment(v: Vertex) -> @location(0) vec4f {
        let c = textureSampleBaseClampToEdge(image, imageSampler, v.uv);
        return c;
      }` });
        const pipeline = await device.createRenderPipelineAsync({ layout: 'auto', vertex: { module, entryPoint: 'vertex' }, fragment: { module, entryPoint: 'fragment', targets: [{ format }] }, primitive: { topology: 'triangle-list' } });
        const sampler = device.createSampler({ minFilter: 'linear', magFilter: 'linear' });
        let lost = false;
        let configured = false;
        void device.lost.then(() => { lost = true; });
        return {
            backend: 'webgpu', canvas,
            async paint(frame) {
                if (lost)
                    throw new Error('WebGPU device lost.');
                if (resize(canvas, frame.displayWidth, frame.displayHeight) || !configured) {
                    context.configure({ device, format, alphaMode: 'premultiplied' });
                    configured = true;
                }
                device.pushErrorScope('validation');
                try {
                    const texture = device.importExternalTexture({ source: frame, colorSpace: 'srgb' });
                    const bindings = device.createBindGroup({
                        layout: pipeline.getBindGroupLayout(0),
                        entries: [{ binding: 0, resource: texture }, { binding: 1, resource: sampler }],
                    });
                    const encoder = device.createCommandEncoder();
                    const pass = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), clearValue: { r: 0, g: 0, b: 0, a: 0 }, loadOp: 'clear', storeOp: 'store' }] });
                    pass.setPipeline(pipeline);
                    pass.setBindGroup(0, bindings);
                    pass.draw(3);
                    pass.end();
                    device.queue.submit([encoder.finish()]);
                    await device.queue.onSubmittedWorkDone();
                }
                finally {
                    const error = await device.popErrorScope();
                    if (error !== null)
                        throw new Error(error.message);
                }
                if (lost)
                    throw new Error('WebGPU device lost.');
            },
            dispose() { lost = true; context.unconfigure(); device.destroy(); },
        };
    }
    catch (error) {
        device.destroy();
        throw error;
    }
}
function glPainter(canvas) {
    const gl = canvas.getContext('webgl2', { alpha: true, antialias: false, preserveDrawingBuffer: true, premultipliedAlpha: true });
    if (gl === null)
        throw new Error('WebGL2 is unavailable.');
    const shaders = [];
    let program = null;
    let texture = null;
    const dispose = () => {
        for (const shader of shaders)
            gl.deleteShader(shader);
        gl.deleteProgram(program);
        gl.deleteTexture(texture);
    };
    try {
        const compile = (type, source) => {
            const shader = gl.createShader(type);
            if (shader === null)
                throw new Error('Native shader allocation failed.');
            shaders.push(shader);
            gl.shaderSource(shader, source);
            gl.compileShader(shader);
            if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
                throw new Error(gl.getShaderInfoLog(shader) ?? 'Native shader compilation failed.');
            return shader;
        };
        const vertex = compile(gl.VERTEX_SHADER, '#version 300 es\nprecision highp float;out vec2 uv;void main(){vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2);uv=vec2(p.x,1.-p.y);gl_Position=vec4(p*2.-1.,0,1);}');
        const fragment = compile(gl.FRAGMENT_SHADER, '#version 300 es\nprecision highp float;in vec2 uv;uniform sampler2D image;out vec4 color;void main(){vec4 c=texture(image,uv);color=vec4(c.rgb*c.a,c.a);}');
        program = gl.createProgram();
        texture = gl.createTexture();
        if (program === null || texture === null)
            throw new Error('Native WebGL allocation failed.');
        gl.attachShader(program, vertex);
        gl.attachShader(program, fragment);
        gl.linkProgram(program);
        if (!gl.getProgramParameter(program, gl.LINK_STATUS))
            throw new Error('Native WebGL program link failed.');
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        let width = 0;
        let height = 0;
        let disposed = false;
        return {
            backend: 'webgl2', canvas,
            async paint(frame) {
                if (disposed || gl.isContextLost())
                    throw new Error('WebGL2 context unavailable.');
                resize(canvas, frame.displayWidth, frame.displayHeight);
                gl.viewport(0, 0, canvas.width, canvas.height);
                gl.useProgram(program);
                gl.bindTexture(gl.TEXTURE_2D, texture);
                if (width !== canvas.width || height !== canvas.height) {
                    width = canvas.width;
                    height = canvas.height;
                    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
                }
                gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, frame);
                gl.drawArrays(gl.TRIANGLES, 0, 3);
                if (gl.getError() !== gl.NO_ERROR)
                    throw new Error('Native WebGL frame upload failed.');
            },
            dispose() { disposed = true; dispose(); },
        };
    }
    catch (error) {
        dispose();
        throw error;
    }
}
/**
 * Allocate a painter on a fresh canvas. Auto uses Chromium's Canvas2D compositor.
 * @param backend - Canvas2D by default or an explicit measurement backend.
 * @param makeCanvas - Must return a new canvas for each attempt; context types cannot switch in place.
 * @returns Initialized painter owning its canvas and graphics resources.
 */
export async function createFrameForwardingFramePainter(backend, makeCanvas = () => document.createElement('canvas')) {
    if (backend === 'canvas2d' || backend === 'auto')
        return canvasPainter(makeCanvas());
    if (backend === 'webgl2')
        return glPainter(makeCanvas());
    return gpuPainter(makeCanvas());
}

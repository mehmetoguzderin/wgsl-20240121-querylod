import Metal
import MetalKit
import Foundation
import simd

// Metal Device Initialization
let device = MTLCopyAllDevices()[0]
print(device.name)

// Shader Source
let librarySource = """
#include <metal_stdlib>
#include <metal_texture>
using namespace metal;

constant float2 positions[] = {
    float2( 0.0, -0.8),
    float2( 1.0, -1.0),
    float2(-1.0, -1.0)
};

constant float2 uvs[] = {
    float2(0.5, 0.0),
    float2(1.0, 1.0),
    float2(0.0, 1.0)
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
};

vertex VertexOut vertex_main(uint vid [[vertex_id]]) {
    VertexOut out;
    out.position = float4(positions[vid], 0.0, 1.0);
    out.uv = uvs[vid];
    return out;
}

fragment float4 fragment_main(VertexOut in [[stage_in]],
                              texture2d<float> textureImage [[texture(0)]],
                              sampler textureSampler [[sampler(0)]]) {
    return float4(textureImage.calculate_clamped_lod(textureSampler, in.uv), textureImage.calculate_clamped_lod(textureSampler, in.uv), 2.5, 1.0);
}
"""

// Create a mipmap texture.
func makeMipTexture(for device: MTLDevice, with size: MTLSize) -> MTLTexture? {
    let descriptor = MTLTextureDescriptor()
    
    descriptor.width = size.width
    descriptor.height = size.height
    descriptor.depth = size.depth
    
    let heightLevels = ceil(log2(Double(size.height)))
    let widthLevels = ceil(log2(Double(size.width)))
    let mipCount = (heightLevels > widthLevels) ? heightLevels : widthLevels
    
    descriptor.mipmapLevelCount = Int(mipCount)
    
    return device.makeTexture(descriptor: descriptor)
}

do {
    let library = try device.makeLibrary(source: librarySource, options: nil)
    let vertexFunction = library.makeFunction(name: "vertex_main")
    let fragmentFunction = library.makeFunction(name: "fragment_main")
    
    // Pipeline State
    let pipelineDescriptor = MTLRenderPipelineDescriptor()
    pipelineDescriptor.vertexFunction = vertexFunction
    pipelineDescriptor.fragmentFunction = fragmentFunction
    pipelineDescriptor.colorAttachments[0].pixelFormat = .rgba32Float
    
    let pipelineState = try device.makeRenderPipelineState(descriptor: pipelineDescriptor)
    
    // Command Queue
    guard let commandQueue = device.makeCommandQueue() else {
        fatalError("Unable to create command queue")
    }
    
    // Render Pass Descriptor
    let renderPassDescriptor = MTLRenderPassDescriptor()
    renderPassDescriptor.colorAttachments[0].loadAction = .clear
    renderPassDescriptor.colorAttachments[0].clearColor = MTLClearColorMake(0, 0, 0, 1)
    renderPassDescriptor.colorAttachments[0].storeAction = .store
    
    // Create a texture to render into
    let textureDescriptor = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba32Float, width: 512, height: 512, mipmapped: false)
    textureDescriptor.usage = [.renderTarget, .shaderRead]
    guard let texture = device.makeTexture(descriptor: textureDescriptor) else {
        fatalError("Unable to create texture")
    }
    renderPassDescriptor.colorAttachments[0].texture = texture
    
    let mipTexture = makeMipTexture(for: device, with: MTLSize(width: 512, height: 512, depth: 1))!
    let samplerDescriptor = MTLSamplerDescriptor()
    samplerDescriptor.minFilter = .linear
    samplerDescriptor.magFilter = .linear
    samplerDescriptor.mipFilter = .linear
    samplerDescriptor.maxAnisotropy = 1
    samplerDescriptor.sAddressMode = .clampToEdge
    samplerDescriptor.tAddressMode = .clampToEdge
    samplerDescriptor.rAddressMode = .clampToEdge
    samplerDescriptor.normalizedCoordinates = true
    samplerDescriptor.lodMinClamp = 0
    samplerDescriptor.lodMaxClamp = .greatestFiniteMagnitude
    guard let samplerState = device.makeSamplerState(descriptor: samplerDescriptor) else {
        fatalError("Unable to create sampler state")
    }
    
    // Command Buffer
    guard let commandBuffer = commandQueue.makeCommandBuffer(),
          let renderEncoder = commandBuffer.makeRenderCommandEncoder(descriptor: renderPassDescriptor) else {
        fatalError("Unable to create command buffer or encoder")
    }
    
    // Encoding Commands
    renderEncoder.setRenderPipelineState(pipelineState)
    renderEncoder.setFragmentTexture(mipTexture, index: 0)
    renderEncoder.setFragmentSamplerState(samplerState, index: 0)
    renderEncoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 3)
    renderEncoder.endEncoding()
    
    // Committing Command Buffer
    commandBuffer.commit()
    commandBuffer.waitUntilCompleted()
    
    // Extracting Image Data and Save as PNG
    let bytesPerPixel = 16 // 4 channels x 4 bytes (32 bits) per channel
    let bytesPerRow = 512 * bytesPerPixel
    let region = MTLRegionMake2D(0, 0, 512, 512)
    var imageBytes = [Float](repeating: 0, count: 512 * 512 * bytesPerPixel)
    texture.getBytes(&imageBytes, bytesPerRow: bytesPerRow, from: region, mipmapLevel: 0)
    let data = Data(buffer: UnsafeBufferPointer(start: &imageBytes, count: imageBytes.count))
    let currentDirectoryURL = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
    let fileURL = currentDirectoryURL.appendingPathComponent("output.metal.bin")
    try data.write(to: fileURL)
    print("Saved to \(fileURL.path)")
} catch {
    print("Error occurred during setup: \(error)")
}

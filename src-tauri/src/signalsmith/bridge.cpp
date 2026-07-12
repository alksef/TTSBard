#define SIGNALSMITH_STRETCH_IMPLEMENTATION
#include "signalsmith-stretch.h"

#include <vector>
#include <cstring>
#include <algorithm>
#include <cmath>

namespace {

struct StretchContext {
    signalsmith::stretch::SignalsmithStretch<float> stretch;
    int channels = 0;
    float sampleRate = 0;
    std::vector<std::vector<float>> deinterleavedIn;
    std::vector<std::vector<float>> deinterleavedOut;
    std::vector<float*> inPtrs;
    std::vector<float*> outPtrs;
};

} // anonymous namespace

extern "C" {

void* signalsmith_stretch_create(int channels, float sampleRate) {
    if (channels < 1 || channels > 32 || sampleRate < 8000.0f || sampleRate > 384000.0f) {
        return nullptr;
    }
    try {
        auto* ctx = new StretchContext();
        ctx->channels = channels;
        ctx->sampleRate = sampleRate;
        ctx->stretch.presetDefault(channels, sampleRate);
        ctx->inPtrs.resize(channels);
        ctx->outPtrs.resize(channels);
        return ctx;
    } catch (...) {
        return nullptr;
    }
}

void signalsmith_stretch_destroy(void* ptr) {
    delete static_cast<StretchContext*>(ptr);
}

int signalsmith_stretch_process(
    void* ptr,
    const float* input,
    int inputFrames,
    float* output,
    int outputCapacityFrames,
    float timeFactor,
    float pitchSemitones,
    int preserveFormants)
{
    if (!ptr || !input || !output || inputFrames <= 0 || timeFactor <= 0.0f) {
        return -1;
    }

    auto* ctx = static_cast<StretchContext*>(ptr);
    const int channels = ctx->channels;

    const int expectedOutputFrames = static_cast<int>(std::round(inputFrames / timeFactor));

    if (expectedOutputFrames <= 0 || expectedOutputFrames > outputCapacityFrames) {
        return -2;
    }

    try {
        ctx->stretch.reset();
        ctx->stretch.setTransposeSemitones(pitchSemitones);
        ctx->stretch.setFormantSemitones(
            preserveFormants ? pitchSemitones : 0.0f,
            preserveFormants
        );

        ctx->deinterleavedIn.resize(channels);
        for (int c = 0; c < channels; ++c) {
            ctx->deinterleavedIn[c].resize(inputFrames);
            for (int i = 0; i < inputFrames; ++i) {
                ctx->deinterleavedIn[c][i] = input[i * channels + c];
            }
            ctx->inPtrs[c] = ctx->deinterleavedIn[c].data();
        }

        ctx->deinterleavedOut.resize(channels);
        for (int c = 0; c < channels; ++c) {
            ctx->deinterleavedOut[c].assign(expectedOutputFrames, 0.0f);
            ctx->outPtrs[c] = ctx->deinterleavedOut[c].data();
        }

        ctx->stretch.setTransposeSemitones(pitchSemitones);
        ctx->stretch.setFormantSemitones(
            preserveFormants ? pitchSemitones : 0.0f,
            preserveFormants
        );

        bool ok = ctx->stretch.exact(ctx->inPtrs, inputFrames, ctx->outPtrs, expectedOutputFrames);

        for (int i = 0; i < expectedOutputFrames; ++i) {
            for (int c = 0; c < channels; ++c) {
                output[i * channels + c] = ctx->deinterleavedOut[c][i];
            }
        }

        return ok ? expectedOutputFrames : -3;
    } catch (...) {
        return -4;
    }
}

int signalsmith_stretch_input_latency(void* ptr) {
    if (!ptr) return 0;
    auto* ctx = static_cast<StretchContext*>(ptr);
    return ctx->stretch.inputLatency();
}

int signalsmith_stretch_output_latency(void* ptr) {
    if (!ptr) return 0;
    auto* ctx = static_cast<StretchContext*>(ptr);
    return ctx->stretch.outputLatency();
}

} // extern "C"

#include "pd_view.h"
#include "pd_backend.h"
#include <stdlib.h>
#include <stdio.h>
#include <string.h>

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

struct HexMemoryData {
    unsigned char* data;
    unsigned char* oldData;
    int dataSize;
    int addressSize;
    char startAddress[64];
    char sizeText[64];
    bool requestData;
    uint64_t sa;
    uint64_t size;
    uint64_t exceptionLocation;
};

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void* createInstance(PDUI* uiFuncs, ServiceFunc* serviceFunc) {
    (void)serviceFunc;
    (void)uiFuncs;

    HexMemoryData* user_data = (HexMemoryData*)malloc(sizeof(HexMemoryData));
    memset(user_data, 0, sizeof(HexMemoryData));

    strcpy(user_data->startAddress, "0x00000000");
    strcpy(user_data->sizeText, "1024");

    user_data->sa = 0;
    user_data->size = 1024;

    user_data->data = (unsigned char*)malloc(128 * 1024);
    user_data->addressSize = 4;

    user_data->oldData = (unsigned char*)malloc(128 * 1024);
    user_data->addressSize = 4;

    // clear

    memset(user_data->data, 0xff, 128 * 1024);
    memset(user_data->oldData, 0xff, 128 * 1024);

    return user_data;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void destroyInstance(void* user_data) {
    free(user_data);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void getAddressLine(char* adressText, uint64_t address, int adressSize) {
    switch (adressSize) {
        case 1:
            sprintf(adressText, "0x%02x", (uint8_t)address); break;
        case 2:
            sprintf(adressText, "0x%04x", (uint16_t)address); break;
        case 4:
            sprintf(adressText, "0x%08x", (uint32_t)address); break;
        case 8:
            sprintf(adressText, "0x%16llx", (int64_t)address); break;
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void drawData(HexMemoryData* data, PDUI* uiFuncs, int lineCount, int charsPerLine) {
    uint64_t address = (uint64_t)strtol(data->startAddress, 0, 16);
    int adressSize = data->addressSize;
    uint8_t* memoryData = data->data;
    uint8_t* oldMemoryData = data->oldData;

    if (charsPerLine > 1024)
        charsPerLine = 1024;

    for (int i = 0; i < lineCount; ++i) {
        char addressText[64] = { 0 };

        // Get Address

        getAddressLine(addressText, address, adressSize);

        // Get Hex and chars

        uiFuncs->text_format("%s: ", addressText); uiFuncs->same_line(0, -1);

        PDColor color = PDUI_COLOR(255, 0, 0, 255);

        // Print hex values

        for (int p = 0; p < charsPerLine; ++p) {
            uint8_t c = memoryData[p];
            uint8_t co = oldMemoryData[p];

            if (c == co)
                uiFuncs->text_format("%02x", c);
            else
                uiFuncs->text_colored(color, "%02x", c);

            uiFuncs->same_line(0, -1);
        }

        // print characters

        for (int p = 0; p < charsPerLine; ++p) {
            uint8_t c = memoryData[p];
            uint8_t co = oldMemoryData[p];
            char wc = 0;

            if (c >= 32 && c < 128)
                wc = (char)c;
            else
                wc = '.';

            if (c == co)
                uiFuncs->text_format("%c", wc);
            else
                uiFuncs->text_colored(color, "%c", wc);

            uiFuncs->same_line(0, 0);
        }

        uiFuncs->text("\n");

        address += (uint32_t)charsPerLine;
        memoryData += charsPerLine;
        oldMemoryData += charsPerLine;
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

void drawUI(HexMemoryData* data, PDUI* uiFuncs) {
    uiFuncs->push_item_width(128);
    uiFuncs->input_text("Start Address", data->startAddress, sizeof(data->startAddress), PDUIInputTextFlags_CharsHexadecimal, 0, 0);
    uiFuncs->same_line(0, -1);
    uiFuncs->input_text("Size", data->sizeText, sizeof(data->sizeText), 0, 0, 0);
    uiFuncs->pop_item_width();

    PDVec2 child_size = { 0.0f, 0.0f };

    long startAddress = strtol(data->startAddress, 0, 16);
    long size = strtol(data->sizeText, 0, 16);

    if (!data->data)
        return;

    if (data->sa != (uint64_t)startAddress) {
        data->requestData = true;
        data->sa = (uint64_t)startAddress;
    }

    if (data->size != (uint64_t)size) {
        data->requestData = true;
        data->size = (uint64_t)size;
    }

    //PDVec2 textStart = uiFuncs->get_cursor_pos();
    PDVec2 windowSize = uiFuncs->get_window_size();

    uiFuncs->begin_child("child", child_size, false, 0);

    //PDRect rect = uiFuncs->getCurrentClipRect();
    //PDVec2 pos = uiFuncs->get_window_pos();

    //printf("pos %f %f\n", pos.x, pos.y);
    //printf("rect %f %f %f %f\n", rect.x, rect.y, rect.width, rect.height);

    // TODO: Fix me
    const float fontWidth = 13.0f; // uiFuncs->getFontWidth();

    float drawableChars = (float)(int)(windowSize.x / (fontWidth + 23));

    int drawableLineCount = (int)((size) / (int)drawableChars);

    //printf("%d %d %d %d\n", drawableLineCount, (int)endAddress, (int)startAddress, (int)drawableChars);

    drawData(data, uiFuncs, drawableLineCount, (int)drawableChars);

    uiFuncs->end_child();
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void updateMemory(HexMemoryData* user_data, PDReader* reader) {
    void* data;
    uint64_t address = 0;
    uint64_t size = user_data->size;

    PDRead_find_u64(reader, &address, "address", 0);

    if (PDRead_find_data(reader, &data, &size, "data", 0) == PDReadStatus_NotFound)
        return;

    user_data->sa = address;

    // save the old data that is used for showing the changes
    memcpy(user_data->oldData, user_data->data, (size_t)size);

    // And update with the new data
    memcpy(user_data->data, data, (size_t)size);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static void updateExceptionLocation(HexMemoryData* data, PDReader* reader) {
    uint64_t address = 0;

    PDRead_find_u64(reader, &address, "address", 0);

    if (data->exceptionLocation == address)
        return;

    data->requestData = true;
    data->exceptionLocation = address;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static int update(void* user_data, PDUI* uiFuncs, PDReader* inEvents, PDWriter* writer) {
    uint32_t event;

    HexMemoryData* data = (HexMemoryData*)user_data;

    data->requestData = false;

    // Loop over all the in events

    while ((event = PDRead_get_event(inEvents)) != 0) {
        switch (event) {
            case PDEventType_SetMemory:
            {
                updateMemory(data, inEvents);
                break;
            }

            case PDEventType_SetExceptionLocation:
            {
                updateExceptionLocation(data, inEvents);
                break;
            }
        }
    }

    drawUI(data, uiFuncs);

    if (data->requestData) {
        //printf("requesting memory range %04x - %04x\n", (uint16_t)data->sa, (uint16_t)data->ea);
        PDWrite_event_begin(writer, PDEventType_GetMemory);
        PDWrite_u64(writer, "address_start", data->sa);
        PDWrite_u64(writer, "size", data->size);
        PDWrite_event_end(writer);
    }

    return 0;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static int saveState(void* user_data, struct PDSaveState* saveState) {
    HexMemoryData* data = (HexMemoryData*)user_data;

    PDIO_write_string(saveState, data->startAddress);
    PDIO_write_string(saveState, data->sizeText);

    return 1;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static int loadState(void* user_data, struct PDLoadState* loadState) {
    HexMemoryData* data = (HexMemoryData*)user_data;

    PDIO_read_string(loadState, data->startAddress, sizeof(data->startAddress));
    PDIO_read_string(loadState, data->sizeText, sizeof(data->sizeText));

    return 1;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

static PDViewPlugin plugin =
{
    "Hex Memory View",
    createInstance,
    destroyInstance,
    update,
    saveState,
    loadState,
};

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

extern "C"
{

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

PD_EXPORT void InitPlugin(RegisterPlugin* registerPlugin, void* private_data) {
	registerPlugin(PD_VIEW_API_VERSION, &plugin, private_data);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

}


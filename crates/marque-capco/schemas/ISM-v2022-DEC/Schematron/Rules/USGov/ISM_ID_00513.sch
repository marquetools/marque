<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00513">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00513][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
        @ism:handleViaChannels is specified, then @ism:secondBannerLine MUST contain the name token [HVCO].
        
        Human Readable: USA documents that specify Handle Via Channels MUST specify [HVCO] in the @ism:secondBannerLine attribute.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which has 
        attribute @ism:handleViaChannels, the element MUST have @ism:secondBannerLine specified with a value containing
        the token [HVCO].
    </sch:p>
    <sch:rule id="ISM-ID-00513-R1" context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and not(util:containsAnyOfTheTokens(@ism:secondBannerLine, 'HVCO'))]">
        <sch:assert test="not(@ism:handleViaChannels)" flag="error" role="error">
            [ISM-ID-00513][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
            @ism:handleViaChannels is specified, then @ism:secondBannerLine MUST contain the name token [HVCO].
            
            Human Readable: USA documents that specify Handle Via Channels MUST specify [HVCO] in the @ism:secondBannerLine attribute.
        </sch:assert>
    </sch:rule>
</sch:pattern>
<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00512">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00512][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
        @ism:secondBannerLine contains the name token [HVCO], then attribute @ism:handleViaChannels must be specified.
        
        Human Readable: USA documents containing Handle Via Channels Only in the second banner line
        must specify to which channels the document is restricted.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, for each element which has 
        attribute @ism:secondBannerLine specified with a value containing
        the token [HVCO] this rule ensures that attribute @ism:handleViaChannels is specified.
    </sch:p>
    <sch:rule id="ISM-ID-00512-R1" context="*[($ISM_USGOV_RESOURCE or $ISM_USCUIONLY_RESOURCE) and util:containsAnyOfTheTokens(@ism:secondBannerLine, 'HVCO')]">
        <sch:assert test="@ism:handleViaChannels" flag="error" role="error">
            [ISM-ID-00512][Error] If ISM_USGOV_RESOURCE or ISM_USCUIONLY_RESOURCE, and attribute 
            @ism:secondBannerLine contains the name token [HVCO], then attribute @ism:handleViaChannels must be specified.
            
            Human Readable: USA documents containing Handle Via Channels Only in the second banner line
            must specify to which channels the document is restricted.
        </sch:assert>
    </sch:rule>
</sch:pattern>
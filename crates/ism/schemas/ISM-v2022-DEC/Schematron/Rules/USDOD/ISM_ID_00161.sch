<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION STRUCTURECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00161">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00161][Error] If the document is an
        1. ISM_USDOD_RESOURCE AND
        2. the attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A] AND
        3. no portions in the document have their attribute @ism:excludeFromRollup set to [true]
        THEN there must not be any attribute @ism:nonICmarkings present.
        
        Human Readable: Distribution statement A (Public Release) is 
        incompatible with any nonICMarkings if excludeFromRollup is not TRUE.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USDOD_RESOURCE and @ism:noticeType contains 'DoD-Dist-A' 
        and no portions in the document have their @ism:excludeFromRollup set to true, 
        then there must not be any @ism:nonICMarkings present.
    </sch:p>
	<sch:rule id="ISM-ID-00161-R1" context="*[$ISM_USDOD_RESOURCE and (util:containsAnyOfTheTokens($ISM_RESOURCE_ELEMENT/@ism:noticeType, ('DoD-Dist-A'))) and not (@ism:excludeFromRollup=true())]">
        <sch:assert test="not(@ism:nonICmarkings)" flag="error" role="error"> 
            [ISM-ID-00161][Error] If the document is an
            1. ISM_USDOD_RESOURCE AND
            2. the attribute @ism:noticeType of ISM_RESOURCE_ELEMENT contains [DoD-Dist-A] AND
            3. no portions in the document have their attribute @ism:excludeFromRollup set to [true]
            THEN there must not be any attribute @ism:nonICmarkings present.
            
            Human Readable: Distribution statement A (Public Release) is 
            incompatible with any nonICMarkings if excludeFromRollup is not TRUE.
        </sch:assert>
    </sch:rule>
    
</sch:pattern>
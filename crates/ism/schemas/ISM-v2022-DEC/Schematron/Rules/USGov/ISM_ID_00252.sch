<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00252">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00252][Error] If ISM_RESOURCE_ELEMENT specifies the attribute
        @ism:disseminationControls with a value containing the token [RELIDO], 
        then attribute @ism:nonICmarkings must not be specified with a value containing 
        the token [NNPI]. 
        
        Human Readable: NNPI tokens are not valid for documents that have
        RELIDO at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        For resource elements which have attribute @ism:disseminationControls specified 
        with a value containing the token [RELIDO], this rule ensures that attribute 
        @ism:nonICmarkings is not specified with a value containing the token [NNPI].
    </sch:p>
    <sch:rule id="ISM-ID-00252-R1" context="*[index-of(tokenize(normalize-space(string($ISM_RESOURCE_ELEMENT/@ism:disseminationControls)), ' '),'RELIDO') &gt; 0 and @ism:nonICmarkings]">
        <sch:assert test="not(util:containsAnyTokenMatching(@ism:nonICmarkings, 'NNPI'))" flag="error" role="error">
            [ISM-ID-00252][Error] If ISM_RESOURCE_ELEMENT specifies the attribute
            @ism:disseminationControls with a value containing the token [RELIDO], 
            then attribute @ism:nonICmarkings must not be specified with a value containing 
            the token [NNPI]. 
            
            Human Readable: NNPI tokens are not valid for documents that have
            RELIDO at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>
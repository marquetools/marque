<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00159">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00159][Error] If ISM_USGOV_RESOURCE and:
        1. attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U]
        AND
        2. The attribute @ism:noticeType does contain [DoD-Dist-A] or has attribute @ism:externalNotice with a value of [true].
        
        Human Readable: Distribution statement A (Public Release) is forbidden on classified documents.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE and the attribute
        @ism:classification of ISM_RESOURCE_ELEMENT is not [U], for each element
        which specifies attribute @ism:noticeType this rule ensures that attribute
        @ism:noticeType is not specified with a value containing the token
        [DoD-Dist-A] unless it is an external notice with attribute @ism:externalNotice is [true].
    </sch:p>
    <sch:rule id="ISM-ID-00159-R1" context="*[$ISM_USGOV_RESOURCE and not($ISM_RESOURCE_ELEMENT/@ism:classification = 'U')]">
        <sch:assert test="not(util:containsAnyOfTheTokens(@ism:noticeType, ('DoD-Dist-A'))) or (@ism:externalNotice=true())" flag="error" role="error"> 
            [ISM-ID-00159][Error] If ISM_USGOV_RESOURCE and:
            1. attribute @ism:classification of ISM_RESOURCE_ELEMENT is not [U]
            AND
            2. The attribute @ism:noticeType does contain [DoD-Dist-A] or has attribute @ism:externalNotice with a value of [true].
            
            Human Readable: Distribution statement A (Public Release) is forbidden on classified documents.
        </sch:assert>
    </sch:rule>
</sch:pattern>
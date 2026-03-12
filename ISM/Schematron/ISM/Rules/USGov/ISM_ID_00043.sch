<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00043">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00043][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains the name token [SI], then attribute @ism:classification must have
        a value of [TS], [S], or [C].
        
        Human Readable: A USA document containing Special Intelligence (SI) 
        data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.  
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing the token
        [SI] this rule ensures that attribute @ism:classification is specified with
        a value containing the token [TS], [S], or [C].
    </sch:p>
    <sch:rule id="ISM-ID-00043-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:SCIcontrols, ('SI'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:classification, ('TS', 'S', 'C'))" flag="error" role="error">
            [ISM-ID-00043][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains the name token [SI], then attribute @ism:classification must have
            a value of [TS], [S], or [C].
            
            Human Readable: A USA document containing Special Intelligence (SI) 
            data must be classified CONFIDENTIAL, SECRET, or TOP SECRET.  
        </sch:assert>
    </sch:rule>
</sch:pattern>
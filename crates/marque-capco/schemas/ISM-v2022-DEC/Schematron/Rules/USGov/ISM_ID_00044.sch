<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00044">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00044][Error] If the document is an ISM_USGOV_RESOURCE and the
        attribute @ism:SCIcontrols contain a name token with [SI-G], then the attribute @ism:classification
        must have a value of [TS]. 
        
        Human Readable: A USA document containing Special Intelligence (SI) GAMMA compartment data 
        must be classified TOP SECRET. 
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols with a value containing a token with [SI-G] this rule
        ensures that attribute @ism:classification is specified with a value containing the token [TS].
    </sch:p>
    <sch:rule id="ISM-ID-00044-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^SI-G$'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))" flag="error" role="error">
            [ISM-ID-00044][Error] If the document is an ISM_USGOV_RESOURCE and the
            attribute @ism:SCIcontrols contain a name token with [SI-G], then the attribute @ism:classification
            must have a value of [TS]. 
            
            Human Readable: A USA document containing Special Intelligence (SI) GAMMA compartment data 
            must be classified TOP SECRET. </sch:assert>
    </sch:rule>
</sch:pattern>
<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00372">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00372][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings
        contains the name token [LES-NF] or [SBU-NF], then attribute @ism:disseminationControls
        must not contain the name token [NF], [REL], [EYES], [RELIDO], or [DISPLAYONLY].
        
        Human Readable: LES-NF and SBU-NF are incompatible with other Foreign Disclosure 
        and Release markings.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:nonICmarkings with a value containing the token
        [LES-NF] or [SBU-NF] this rule ensures that attribute @ism:disseminationControls is 
        not specified with a value containing the token [NF], [REL], [EYES], [RELIDO], or 
        [DISPLAYONLY].
    </sch:p>
    <sch:rule id="ISM-ID-00372-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:nonICmarkings, ('LES-NF','SBU-NF'))]">  
        <sch:assert test="not(util:containsAnyOfTheTokens(@ism:disseminationControls, ('NF','REL','EYES','RELIDO','DISPLAYONLY')))" flag="error" role="error">
            [ISM-ID-00372][Error] If ISM_USGOV_RESOURCE and attribute @ism:nonICmarkings
            contains the name token [LES-NF] or [SBU-NF], then attribute @ism:disseminationControls
            must not contain the name token [NF], [REL], [EYES], [RELIDO], or [DISPLAYONLY].
            
            Human Readable: LES-NF and SBU-NF are incompatible with other Foreign Disclosure 
            and Release markings.
        </sch:assert>
    </sch:rule>
</sch:pattern>